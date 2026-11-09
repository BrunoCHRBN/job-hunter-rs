use std::fs;
use std::sync::Arc;
use std::time::Duration;

use anyhow::Result;
use serde::Serialize;
use tokio::process::Command;
use tokio::sync::RwLock;
use tracing::{info, warn};

use crate::collectors;
use crate::config::AppConfig;
use crate::db::{cosine_similarity, Database};
use crate::gemini::GeminiClient;

#[derive(Debug, Clone, Default, Serialize)]
pub struct AutomationRuntimeStatus {
    pub worker_active: bool,
    pub is_running_cycle: bool,
    pub last_cycle_summary: String,
    pub total_auto_collected: usize,
    pub total_auto_enriched: usize,
    pub total_auto_matched: usize,
}

pub type SharedAutomationStatus = Arc<RwLock<AutomationRuntimeStatus>>;

pub fn new_shared_status() -> SharedAutomationStatus {
    Arc::new(RwLock::new(AutomationRuntimeStatus {
        worker_active: true,
        is_running_cycle: false,
        last_cycle_summary: "Worker autônomo ativo em segundo plano.".to_string(),
        total_auto_collected: 0,
        total_auto_enriched: 0,
        total_auto_matched: 0,
    }))
}

pub fn spawn_automation_worker(
    cfg: AppConfig,
    db: Database,
    status: SharedAutomationStatus,
) {
    tokio::spawn(async move {
        info!("🤖 Worker Autônomo (Zero-Touch) iniciado em background.");
        // Aguarda 8 segundos após o boot do servidor antes do primeiro check
        tokio::time::sleep(Duration::from_secs(8)).await;

        let mut interval = tokio::time::interval(Duration::from_secs(30));
        loop {
            interval.tick().await;
            if let Err(err) = run_automation_tick(&cfg, &db, &status, false).await {
                warn!("Erro no ciclo do Worker Autônomo: {}", err);
            }
        }
    });
}

/// Executa um passo do pipeline autônomo:
/// 1. Se `force_all_searches` for `true`, roda todas as buscas ativas (`enabled = 1`);
///    caso contrário, roda apenas as buscas cujo `interval_minutes` venceu (`get_due_saved_searches`).
/// 2. Se `profile.auto_enrich == 1`, enriquece 1 lote de até 5 vagas pendentes com `verify_evidence` + embedding.
/// 3. Calcula similaridade de cosseno das vagas enriquecidas sem match contra `curriculo.md` e,
///    se `sim >= profile.auto_match_threshold`, dispara a análise profunda de Match (`Fit v2`) para até 2 vagas por ciclo.
pub async fn run_automation_tick(
    cfg: &AppConfig,
    db: &Database,
    status: &SharedAutomationStatus,
    force_all_searches: bool,
) -> Result<String> {
    {
        let mut guard = status.write().await;
        if guard.is_running_cycle {
            return Ok("Ciclo já em execução.".to_string());
        }
        guard.is_running_cycle = true;
    }

    let res = execute_pipeline_steps(cfg, db, status, force_all_searches).await;

    {
        let mut guard = status.write().await;
        guard.is_running_cycle = false;
        if let Ok(ref summary) = res {
            guard.last_cycle_summary = summary.clone();
        }
    }

    res
}

async fn execute_pipeline_steps(
    cfg: &AppConfig,
    db: &Database,
    status: &SharedAutomationStatus,
    force_all_searches: bool,
) -> Result<String> {
    let profile = db.get_candidate_profile().await.unwrap_or_default();

    // 1. Rondas de busca programadas
    let searches = if force_all_searches {
        db.list_saved_searches()
            .await?
            .into_iter()
            .filter(|s| s.enabled != 0)
            .collect::<Vec<_>>()
    } else {
        db.get_due_saved_searches().await?
    };

    let mut newly_collected = 0usize;
    let mut searches_ran = 0usize;

    for s in searches {
        let loc_opt = if s.location.trim().is_empty() {
            None
        } else {
            Some(s.location.as_str())
        };
        match collectors::collect_all(cfg, db, &s.query, loc_opt, 15, s.remote != 0).await {
            Ok(rep) => {
                newly_collected += rep.newly_inserted;
                searches_ran += 1;
                let _ = db.mark_saved_search_ran(s.id, rep.newly_inserted).await;
            }
            Err(err) => {
                warn!("Falha na ronda automática '{}': {}", s.query, err);
            }
        }
    }

    // 2. Auto-Enriquecimento (1 lote de até 5 vagas por tick)
    let mut enriched_now = 0usize;
    let mut matched_now = 0usize;

    if let Ok(api_key) = cfg.require_gemini_key() {
        let gemini = GeminiClient::new(api_key)?;

        if profile.auto_enrich != 0 || force_all_searches {
            let unenriched = db.get_unenriched_jobs(5).await?;
            if !unenriched.is_empty() {
                let raw_batch: Vec<_> = unenriched.iter().map(|j| j.to_raw_listing()).collect();
                if let Ok(extracted_list) = gemini.extract_jobs_batch(&raw_batch).await {
                    for (idx, job_record) in unenriched.iter().enumerate() {
                        let expected_ext_id = job_record
                            .external_id
                            .clone()
                            .unwrap_or_else(|| job_record.id.to_string());

                        let maybe_meta = extracted_list
                            .iter()
                            .find(|m| m.external_id == expected_ext_id)
                            .or_else(|| extracted_list.get(idx));

                        let Some(raw_meta) = maybe_meta else {
                            continue;
                        };

                        let mut meta = raw_meta.clone();
                        meta.dossier.verify_evidence(
                            job_record.raw_description.as_deref().unwrap_or(""),
                        );

                        let embedding_text = job_record.build_embedding_text(&meta);
                        let embedding_opt = gemini.generate_embedding(&embedding_text).await.ok();

                        db.update_job_enrichment(
                            job_record.id,
                            &meta,
                            embedding_opt.as_deref(),
                        )
                        .await?;
                        enriched_now += 1;
                    }
                }
            }
        }

        // 3. Auto-Match Seletivo guiado por similaridade semântica (cosseno >= threshold)
        if let Ok(resume_md) = fs::read_to_string(&cfg.resume_path) {
            let unmatched = db.get_unmatched_enriched_jobs(25).await?;
            if !unmatched.is_empty() {
                if let Ok(resume_emb) = gemini.generate_embedding(&resume_md).await {
                    let threshold = profile.auto_match_threshold as f32;
                    let mut candidates: Vec<_> = unmatched
                        .into_iter()
                        .filter_map(|job| {
                            let emb = job.embedding()?;
                            let sim = cosine_similarity(&resume_emb, &emb);
                            if sim >= threshold {
                                Some((job, sim))
                            } else {
                                None
                            }
                        })
                        .collect();

                    candidates.sort_by(|a, b| {
                        b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal)
                    });

                    // Processa até 2 auto-matches por ciclo para respeitar rate-limits
                    for (job, sim) in candidates.into_iter().take(2) {
                        info!(
                            "🎯 Auto-Match acionado para vaga #{} ({} — sim: {:.1}% >= {:.1}%)",
                            job.id,
                            job.title_str(),
                            sim * 100.0,
                            threshold * 100.0
                        );
                        if let Ok(analysis) = gemini
                            .analyze_job_match_with_profile(&resume_md, &job, &profile)
                            .await
                        {
                            let verdict = analysis.verdict_kind();
                            db.update_job_match(job.id, &analysis).await?;
                            matched_now += 1;

                            if profile.notify_on_apply != 0 && verdict == "apply" {
                                send_desktop_notification(
                                    &format!(
                                        "Vaga Recomendada ({:.0}%): {}",
                                        analysis.match_score,
                                        job.title_str()
                                    ),
                                    &format!(
                                        "{} · {} — Abra http://localhost:8787/#vagas/{}",
                                        job.company_str(),
                                        job.location_str(),
                                        job.id
                                    ),
                                )
                                .await;
                            }
                        }
                    }
                }
            }
        }
    }

    {
        let mut guard = status.write().await;
        guard.total_auto_collected += newly_collected;
        guard.total_auto_enriched += enriched_now;
        guard.total_auto_matched += matched_now;
    }

    let summary = if searches_ran == 0 && enriched_now == 0 && matched_now == 0 {
        "Monitoramento em dia (nenhuma pendência no momento).".to_string()
    } else {
        format!(
            "Rondas: {} executada(s), +{} vaga(s) nova(s), {} enriquecida(s), {} auto-match(es).",
            searches_ran, newly_collected, enriched_now, matched_now
        )
    };

    Ok(summary)
}

async fn send_desktop_notification(title: &str, body: &str) {
    let _ = Command::new("notify-send")
        .arg("--app-name=Job Hunter RS")
        .arg(title)
        .arg(body)
        .output()
        .await;
}
