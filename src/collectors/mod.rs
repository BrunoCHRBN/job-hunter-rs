pub mod ats;
pub mod brasil;
pub mod gupy;
pub mod jobspy;

use std::collections::HashSet;
use std::time::Instant;

use anyhow::Result;
use serde::{Deserialize, Serialize};
use tracing::{info, warn};

use crate::collectors::brasil::{SourceDiagnosticReport, SourceDiagnosticStatus};
use crate::config::AppConfig;
use crate::db::Database;
use crate::execution::ExecutionHandle;
use crate::models::RawJobListing;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CollectionReport {
    pub gupy_count: usize,
    pub jobspy_count: usize,
    pub linkedin_count: usize,
    pub indeed_count: usize,
    pub glassdoor_count: usize,
    pub vagas_com_count: usize,
    pub ats_count: usize,
    pub total_fetched: usize,
    pub excluded_by_filter: usize,
    pub newly_inserted: usize,
    pub duplicates_skipped: usize,
    pub warnings: Vec<String>,
    pub source_diagnostics: Vec<SourceDiagnosticReport>,
}

/// Orquestra a busca em todas as fontes habilitadas (compatível com chamadas existentes).
pub async fn collect_all(
    config: &AppConfig,
    db: &Database,
    query: &str,
    location: Option<&str>,
    limit: usize,
    remote: bool,
    exec_handle: Option<&ExecutionHandle>,
) -> Result<CollectionReport> {
    collect_with_sources(config, db, query, location, limit, remote, None, exec_handle).await
}

/// Search Orchestrator completo (Sprint 09):
/// - Respeita fontes selecionadas manualmente (`selected_sources`) ou as configurações de `collector_configs`.
/// - Executa os coletores habilitados concorrentemente.
/// - Padroniza o diagnóstico por fonte (`SUCCESS`, `EMPTY`, `BLOCKED`, `ERROR`, `TIMEOUT`, `DISABLED`).
/// - Consolida resultados e encaminha para normalização, deduplicação multi-nível e triagem determinística.
#[allow(clippy::too_many_arguments)]
pub async fn collect_with_sources(
    config: &AppConfig,
    db: &Database,
    query: &str,
    location: Option<&str>,
    limit: usize,
    remote: bool,
    selected_sources: Option<&[String]>,
    exec_handle: Option<&ExecutionHandle>,
) -> Result<CollectionReport> {
    let loc_str = location.unwrap_or("Brasil");
    let profile = db.get_candidate_profile().await.unwrap_or_default();
    let collector_configs = db.list_collectors().await.unwrap_or_default();

    let enabled_set: HashSet<String> = if let Some(sel) = selected_sources {
        if sel.is_empty() || sel.iter().any(|s| s.eq_ignore_ascii_case("all") || s.eq_ignore_ascii_case("todas")) {
            collector_configs
                .iter()
                .filter(|c| c.enabled == 1)
                .map(|c| c.source_id.clone())
                .collect()
        } else {
            sel.iter().map(|s| s.to_lowercase()).collect()
        }
    } else {
        collector_configs
            .iter()
            .filter(|c| c.enabled == 1)
            .map(|c| c.source_id.clone())
            .collect()
    };

    let is_enabled = |id: &str| -> bool {
        if collector_configs.is_empty() {
            matches!(id, "gupy" | "linkedin" | "indeed" | "glassdoor" | "vagas_com")
        } else {
            enabled_set.contains(id)
        }
    };

    if let Some(h) = exec_handle {
        h.emit_info(
            "setup",
            Some("orchestrator"),
            &format!(
                "Search Orchestrator iniciado: query='{}', local='{}', remoto={}, limite={}, fontes_ativas={}",
                query,
                loc_str,
                remote,
                limit,
                enabled_set.len()
            ),
        )
        .await;
        let _ = h.update_progress(10.0, Some("coleta_fontes"), 0, 0).await;
    }

    let fetch_stage = if let Some(h) = exec_handle {
        Some(h.start_stage("coleta_fontes").await?)
    } else {
        None
    };

    let run_gupy = is_enabled("gupy");
    let run_jobspy = is_enabled("linkedin") || is_enabled("indeed") || is_enabled("glassdoor");
    let run_vagas_com = is_enabled("vagas_com");
    let run_solides = is_enabled("solides");

    let gupy_started = Instant::now();
    let gupy_fut = async {
        if run_gupy {
            Some(gupy::fetch_gupy_jobs(query, location, limit, remote, exec_handle).await)
        } else {
            None
        }
    };

    let jobspy_started = Instant::now();
    let jobspy_fut = async {
        if run_jobspy {
            Some(
                jobspy::fetch_jobspy_jobs(
                    &config.python_bin,
                    query,
                    loc_str,
                    limit,
                    168,
                    remote,
                    exec_handle,
                )
                .await,
            )
        } else {
            None
        }
    };

    let vagas_fut = async {
        if run_vagas_com {
            Some(brasil::fetch_vagas_com_jobs(query, location, limit).await)
        } else {
            None
        }
    };

    let solides_fut = async {
        if run_solides {
            Some(brasil::fetch_solides_jobs(query, location, limit).await)
        } else {
            None
        }
    };

    let (gupy_opt, jobspy_opt, vagas_opt, solides_opt) =
        tokio::join!(gupy_fut, jobspy_fut, vagas_fut, solides_fut);

    let mut warnings = Vec::new();
    let mut source_diagnostics = Vec::new();
    let mut combined: Vec<RawJobListing> = Vec::new();

    // 1. Diagnóstico Gupy
    let gupy_count = match gupy_opt {
        Some(Ok(jobs)) => {
            let cnt = jobs.len();
            let status = if cnt > 0 {
                SourceDiagnosticStatus::Success
            } else {
                SourceDiagnosticStatus::Empty
            };
            let diag = SourceDiagnosticReport {
                source_id: "gupy".to_string(),
                display_name: "Gupy Portal & SSR".to_string(),
                status,
                count: cnt,
                duration_ms: gupy_started.elapsed().as_millis() as u64,
                message: format!("{} vagas retornadas pela Gupy", cnt),
            };
            let _ = db
                .update_collector_run_status("gupy", status.as_str(), cnt, None)
                .await;
            source_diagnostics.push(diag);
            combined.extend(jobs);
            cnt
        }
        Some(Err(err)) => {
            let msg = format!("Falha na Gupy: {}", err);
            warn!("{}", msg);
            warnings.push(msg.clone());
            let _ = db
                .update_collector_run_status("gupy", "ERROR", 0, Some(&msg))
                .await;
            source_diagnostics.push(SourceDiagnosticReport {
                source_id: "gupy".to_string(),
                display_name: "Gupy Portal & SSR".to_string(),
                status: SourceDiagnosticStatus::Error,
                count: 0,
                duration_ms: gupy_started.elapsed().as_millis() as u64,
                message: msg,
            });
            0
        }
        None => {
            source_diagnostics.push(SourceDiagnosticReport {
                source_id: "gupy".to_string(),
                display_name: "Gupy Portal & SSR".to_string(),
                status: SourceDiagnosticStatus::Disabled,
                count: 0,
                duration_ms: 0,
                message: "Coletor desativado para esta busca".to_string(),
            });
            0
        }
    };

    // 2. Diagnóstico JobSpy (LinkedIn, Indeed, Glassdoor)
    let (mut linkedin_count, mut indeed_count, mut glassdoor_count, mut jobspy_count) = (0, 0, 0, 0);
    if let Some(js_res) = jobspy_opt {
        let js_dur = jobspy_started.elapsed().as_millis() as u64;
        warnings.extend(js_res.warnings.clone());

        for job in js_res.jobs {
            let src_low = job.source.to_lowercase();
            if src_low.contains("linkedin") && !is_enabled("linkedin") {
                continue;
            }
            if src_low.contains("indeed") && !is_enabled("indeed") {
                continue;
            }
            if src_low.contains("glassdoor") && !is_enabled("glassdoor") {
                continue;
            }
            if src_low.contains("linkedin") {
                linkedin_count += 1;
            } else if src_low.contains("indeed") {
                indeed_count += 1;
            } else if src_low.contains("glassdoor") {
                glassdoor_count += 1;
            }
            jobspy_count += 1;
            combined.push(job);
        }

        for (site_id, site_label, cnt) in [
            ("linkedin", "LinkedIn (JobSpy)", linkedin_count),
            ("indeed", "Indeed Brasil (JobSpy)", indeed_count),
            ("glassdoor", "Glassdoor Brasil (JobSpy)", glassdoor_count),
        ] {
            if !is_enabled(site_id) {
                source_diagnostics.push(SourceDiagnosticReport {
                    source_id: site_id.to_string(),
                    display_name: site_label.to_string(),
                    status: SourceDiagnosticStatus::Disabled,
                    count: 0,
                    duration_ms: 0,
                    message: "Coletor desativado".to_string(),
                });
                continue;
            }
            let site_err = js_res
                .warnings
                .iter()
                .find(|w| w.to_lowercase().contains(site_id))
                .cloned();
            let status = if cnt > 0 {
                SourceDiagnosticStatus::Success
            } else if let Some(ref e) = site_err {
                if e.contains("403") || e.contains("429") || e.to_lowercase().contains("captcha") {
                    SourceDiagnosticStatus::Blocked
                } else {
                    SourceDiagnosticStatus::Error
                }
            } else {
                SourceDiagnosticStatus::Empty
            };
            let _ = db
                .update_collector_run_status(
                    site_id,
                    status.as_str(),
                    cnt,
                    site_err.as_deref(),
                )
                .await;
            source_diagnostics.push(SourceDiagnosticReport {
                source_id: site_id.to_string(),
                display_name: site_label.to_string(),
                status,
                count: cnt,
                duration_ms: js_dur,
                message: site_err.unwrap_or_else(|| format!("{} vagas coletadas em {}", cnt, site_label)),
            });
        }
    } else {
        for (site_id, site_label) in [
            ("linkedin", "LinkedIn (JobSpy)"),
            ("indeed", "Indeed Brasil (JobSpy)"),
            ("glassdoor", "Glassdoor Brasil (JobSpy)"),
        ] {
            source_diagnostics.push(SourceDiagnosticReport {
                source_id: site_id.to_string(),
                display_name: site_label.to_string(),
                status: SourceDiagnosticStatus::Disabled,
                count: 0,
                duration_ms: 0,
                message: "Coletor desativado".to_string(),
            });
        }
    }

    // 3. Diagnóstico Vagas.com.br
    let vagas_com_count = if let Some((v_jobs, v_diag)) = vagas_opt {
        let cnt = v_jobs.len();
        let err_opt = if matches!(
            v_diag.status,
            SourceDiagnosticStatus::Error | SourceDiagnosticStatus::Blocked | SourceDiagnosticStatus::Timeout
        ) {
            warnings.push(v_diag.message.clone());
            Some(v_diag.message.as_str())
        } else {
            None
        };
        let _ = db
            .update_collector_run_status("vagas_com", v_diag.status.as_str(), cnt, err_opt)
            .await;
        if let Some(h) = exec_handle {
            h.emit_info(
                "coleta_fontes",
                Some("vagas_com"),
                &format!(
                    "Vagas.com.br finalizou com status {} ({} vagas em {}ms)",
                    v_diag.status.as_str(),
                    cnt,
                    v_diag.duration_ms
                ),
            )
            .await;
        }
        source_diagnostics.push(v_diag);
        combined.extend(v_jobs);
        cnt
    } else {
        source_diagnostics.push(SourceDiagnosticReport {
            source_id: "vagas_com".to_string(),
            display_name: "Vagas.com.br".to_string(),
            status: SourceDiagnosticStatus::Disabled,
            count: 0,
            duration_ms: 0,
            message: "Coletor desativado".to_string(),
        });
        0
    };

    // 4. Diagnóstico Sólides
    if let Some((s_jobs, s_diag)) = solides_opt {
        let cnt = s_jobs.len();
        let _ = db
            .update_collector_run_status("solides", s_diag.status.as_str(), cnt, None)
            .await;
        source_diagnostics.push(s_diag);
        combined.extend(s_jobs);
    }

    let ats_count = 0usize;
    let total_fetched = combined.len();

    if let Some(ref stage) = fetch_stage {
        stage
            .complete((limit * 5) as i64, total_fetched as i64)
            .await
            .ok();
    }

    if let Some(h) = exec_handle {
        let _ = h
            .update_progress(
                55.0,
                Some("triagem_e_deduplicacao"),
                total_fetched as i64,
                0,
            )
            .await;
        h.emit_info(
            "coleta_fontes",
            Some("orchestrator"),
            &format!(
                "Fontes consultadas: {} vagas brutas (Gupy: {}, JobSpy: {} [LI: {}, IN: {}, GD: {}], Vagas.com: {})",
                total_fetched,
                gupy_count,
                jobspy_count,
                linkedin_count,
                indeed_count,
                glassdoor_count,
                vagas_com_count
            ),
        )
        .await;
    }

    let dedup_stage = if let Some(h) = exec_handle {
        Some(h.start_stage("triagem_e_deduplicacao").await?)
    } else {
        None
    };

    let filtered: Vec<RawJobListing> = combined
        .into_iter()
        .filter(|j| !profile.matches_exclusion(&j.title))
        .collect();
    let excluded_by_filter = total_fetched.saturating_sub(filtered.len());

    let (newly_inserted, duplicates_skipped) = db.insert_raw_jobs_batch(&filtered).await?;

    if let Some(ref stage) = dedup_stage {
        stage
            .complete(total_fetched as i64, newly_inserted as i64)
            .await
            .ok();
    }

    info!(
        "Coleta concluída: {} obtidas (Gupy: {}, JobSpy: {}, Vagas.com: {}), {} filtradas, {} novas salvas, {} duplicadas consolidadas.",
        total_fetched,
        gupy_count,
        jobspy_count,
        vagas_com_count,
        excluded_by_filter,
        newly_inserted,
        duplicates_skipped
    );

    if let Some(h) = exec_handle {
        let _ = h
            .update_progress(95.0, Some("conclusao"), newly_inserted as i64, 0)
            .await;
        h.emit_info(
            "triagem_e_deduplicacao",
            Some("orchestrator"),
            &format!(
                "Normalização, deduplicação multi-nível e triagem determinística concluídas: {} novas vagas, {} origens unificadas, {} excluídas",
                newly_inserted, duplicates_skipped, excluded_by_filter
            ),
        )
        .await;
        h.record_metric("newly_inserted_jobs", newly_inserted as f64, "count")
            .await
            .ok();
        h.record_metric("duplicates_skipped_jobs", duplicates_skipped as f64, "count")
            .await
            .ok();
        h.record_metric("excluded_by_filter_jobs", excluded_by_filter as f64, "count")
            .await
            .ok();
    }

    Ok(CollectionReport {
        gupy_count,
        jobspy_count,
        linkedin_count,
        indeed_count,
        glassdoor_count,
        vagas_com_count,
        ats_count,
        total_fetched,
        excluded_by_filter,
        newly_inserted,
        duplicates_skipped,
        warnings,
        source_diagnostics,
    })
}
