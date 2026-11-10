use std::fs;
use std::net::SocketAddr;
use std::sync::Arc;

use anyhow::{Context, Result};
use axum::body::Bytes;
use axum::extract::{DefaultBodyLimit, Path, Query, State};
use axum::http::{header, StatusCode};
use axum::response::sse::{Event as SseEvent, KeepAlive, Sse};
use axum::response::{IntoResponse, Response};
use axum::routing::{delete, get, post};
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use serde_json::json;
use tokio_stream::wrappers::BroadcastStream;
use tokio_stream::StreamExt;
use tracing::{info, warn};

use crate::automation::{self, SharedAutomationStatus};
use crate::collectors;
use crate::config::AppConfig;
use crate::db::Database;
use crate::execution::{
    ExecutionEngine, ExecutionEventMessage, ExecutionHandle, ExecutionType,
};
use crate::gemini::GeminiClient;
use crate::models::{CandidateProfile, JobFilter, JobRecord};

const INDEX_HTML: &str = include_str!("../static/index.html");
const STYLES_CSS: &str = include_str!("../static/styles.css");
const IDENTITY_CSS: &str = include_str!("../static/identity.css");
const APP_JS: &str = include_str!("../static/app.js");

#[derive(Clone)]
pub struct WebState {
    pub cfg: AppConfig,
    pub db: Database,
    pub engine: Arc<ExecutionEngine>,
    pub auto_status: SharedAutomationStatus,
}

#[derive(Debug, Deserialize, Default)]
pub struct JobsQueryParams {
    pub work_model: Option<String>,
    pub seniority: Option<String>,
    pub shift: Option<String>,
    pub status: Option<String>,
    pub source: Option<String>,
    pub screening_verdict: Option<String>,
    pub company_id: Option<i64>,
    pub q: Option<String>,
}

#[derive(Debug, Deserialize, Default)]
pub struct ExecutionsQueryParams {
    pub limit: Option<i64>,
    pub offset: Option<i64>,
    pub r#type: Option<String>,
    pub status: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct CollectPayload {
    pub query: String,
    pub location: Option<String>,
    #[serde(default = "default_limit")]
    pub limit: usize,
    #[serde(default)]
    pub remote: bool,
    #[serde(default)]
    pub enrich: bool,
    #[serde(default)]
    pub sources: Option<Vec<String>>,
}

fn default_limit() -> usize {
    20
}

#[derive(Debug, Deserialize, Default)]
pub struct EnrichPayload {
    #[serde(default = "default_batch_size")]
    pub batch_size: usize,
}

fn default_batch_size() -> usize {
    5
}

#[derive(Debug, Deserialize)]
pub struct StatusUpdatePayload {
    pub status: String,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct ResumePayload {
    pub content: String,
}

#[derive(Debug, Deserialize)]
pub struct NewSavedSearchPayload {
    pub query: String,
    #[serde(default)]
    pub location: String,
    #[serde(default)]
    pub remote: bool,
    #[serde(default = "default_interval_minutes")]
    pub interval_minutes: i64,
}

fn default_interval_minutes() -> i64 {
    180
}

#[derive(Debug, Deserialize)]
pub struct ToggleSavedSearchPayload {
    pub enabled: bool,
}

#[derive(Debug, Deserialize)]
pub struct UpsertCompanyPayload {
    pub official_name: String,
    #[serde(default)]
    pub website: String,
    #[serde(default)]
    pub segment: String,
    #[serde(default)]
    pub location: String,
    #[serde(default)]
    pub description: String,
    #[serde(default = "default_true")]
    pub is_monitored: bool,
    #[serde(default)]
    pub is_preferred: bool,
    #[serde(default)]
    pub is_excluded: bool,
    #[serde(default)]
    pub ats_provider: Option<String>,
    #[serde(default)]
    pub portal_id: Option<String>,
    #[serde(default)]
    pub career_url: Option<String>,
}

fn default_true() -> bool {
    true
}

#[derive(Debug, Deserialize)]
pub struct AddAliasPayload {
    pub alias_name: String,
}

#[derive(Debug, Deserialize)]
pub struct AddCompanySourcePayload {
    #[serde(default = "default_source_type")]
    pub source_type: String,
    pub career_url: String,
    pub ats_provider: String,
    pub portal_id: String,
}

fn default_source_type() -> String {
    "ATS_BOARD".to_string()
}

#[derive(Debug, Deserialize, Default)]
pub struct CompanyScanPayload {
    pub query: Option<String>,
    #[serde(default = "default_company_limit")]
    pub limit: usize,
}

fn default_company_limit() -> usize {
    30
}

pub async fn run_web_server(
    cfg: AppConfig,
    db: Database,
    engine: Arc<ExecutionEngine>,
    port: u16,
) -> Result<()> {
    let auto_status = automation::new_shared_status();
    automation::spawn_automation_worker(
        cfg.clone(),
        db.clone(),
        engine.clone(),
        auto_status.clone(),
    );

    let state = Arc::new(WebState {
        cfg,
        db,
        engine,
        auto_status,
    });

    let app = build_router(state);

    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    info!("Servidor Web iniciado em http://127.0.0.1:{}", port);
    println!("Bancada Web Job Hunter RS ativa em: http://localhost:{}", port);

    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .with_context(|| format!("Falha ao fazer bind na porta {}", port))?;

    axum::serve(listener, app)
        .await
        .context("Erro durante execução do servidor HTTP Axum")?;

    Ok(())
}

pub fn build_router(state: Arc<WebState>) -> Router {
    Router::new()
        .route("/", get(serve_index))
        .route("/index.html", get(serve_index))
        .route("/styles.css", get(serve_css))
        .route("/static/styles.css", get(serve_css))
        .route("/identity.css", get(serve_identity_css))
        .route("/static/identity.css", get(serve_identity_css))
        .route("/app.js", get(serve_js))
        .route("/static/app.js", get(serve_js))
        .route("/api/stats", get(api_stats))
        .route("/api/jobs", get(api_list_jobs))
        .route("/api/jobs/:id", get(api_get_job))
        .route("/api/jobs/fetch", post(api_collect))
        .route("/api/search", post(api_collect))
        .route("/api/collect", post(api_collect))
        .route("/api/jobs/enrich", post(api_enrich))
        .route("/api/enrich", post(api_enrich))
        .route("/api/rank", post(api_rank))
        .route("/api/jobs/screen-all", post(api_screen_all_jobs))
        .route("/api/jobs/consolidate-duplicates", post(api_consolidate_duplicates))
        .route("/api/jobs/:id/screen", post(api_screen_job))
        .route("/api/jobs/:id/sources", get(api_get_job_sources))
        .route("/api/jobs/:id/match", post(api_job_match))
        .route(
            "/api/jobs/:id/status",
            post(api_update_status).patch(api_update_status),
        )
        .route("/api/collectors", get(api_list_collectors))
        .route("/api/collectors/:id/toggle", post(api_toggle_collector))
        .route(
            "/api/companies",
            get(api_list_companies).post(api_upsert_company),
        )
        .route("/api/companies/:id", get(api_get_company_details))
        .route("/api/companies/:id/aliases", post(api_add_company_alias))
        .route("/api/companies/:id/sources", post(api_add_company_source))
        .route("/api/companies/:id/scan", post(api_scan_company))
        .route("/api/ai/telemetry", get(api_ai_telemetry))
        .route("/api/resume", get(api_get_resume).put(api_put_resume))
        .route(
            "/api/resume/upload-pdf",
            post(api_upload_resume_pdf).layer(DefaultBodyLimit::max(15 * 1024 * 1024)),
        )
        .route("/api/profile", get(api_get_profile).put(api_put_profile))
        .route("/api/automation", get(api_get_automation))
        .route("/api/automation/searches", post(api_create_saved_search))
        .route(
            "/api/automation/searches/:id/toggle",
            post(api_toggle_saved_search),
        )
        .route(
            "/api/automation/searches/:id",
            delete(api_delete_saved_search),
        )
        .route("/api/automation/run-now", post(api_run_automation_now))
        .route("/api/executions", get(api_list_executions))
        .route("/api/executions/summary", get(api_get_executions_summary))
        .route("/api/executions/stats", get(api_get_executions_summary))
        .route("/api/executions/:id", get(api_get_execution))
        .route("/api/executions/:id/cancel", post(api_cancel_execution))
        .route("/api/executions/:id/retry", post(api_retry_execution))
        .route("/api/executions/stream", get(api_executions_stream))
        .route("/api/executions/:id/stream", get(api_execution_id_stream))
        .with_state(state)
}

async fn serve_index() -> Response {
    (
        [
            (header::CONTENT_TYPE, "text/html; charset=utf-8"),
            (header::CACHE_CONTROL, "no-store, no-cache, must-revalidate"),
        ],
        INDEX_HTML,
    )
        .into_response()
}

async fn serve_css() -> Response {
    (
        [
            (header::CONTENT_TYPE, "text/css; charset=utf-8"),
            (header::CACHE_CONTROL, "no-store, no-cache, must-revalidate"),
        ],
        STYLES_CSS,
    )
        .into_response()
}

async fn serve_identity_css() -> Response {
    (
        [
            (header::CONTENT_TYPE, "text/css; charset=utf-8"),
            (header::CACHE_CONTROL, "no-store, no-cache, must-revalidate"),
        ],
        IDENTITY_CSS,
    )
        .into_response()
}

async fn serve_js() -> Response {
    (
        [
            (
                header::CONTENT_TYPE,
                "application/javascript; charset=utf-8",
            ),
            (header::CACHE_CONTROL, "no-store, no-cache, must-revalidate"),
        ],
        APP_JS,
    )
        .into_response()
}

fn internal_err(err: impl std::fmt::Display) -> (StatusCode, String) {
    (StatusCode::INTERNAL_SERVER_ERROR, err.to_string())
}

async fn api_stats(
    State(state): State<Arc<WebState>>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let stats = state.db.get_stats().await.map_err(internal_err)?;
    Ok(Json(json!({
        "total_jobs": stats.total_jobs,
        "enriched_jobs": stats.enriched_jobs,
        "saved_or_applied_jobs": stats.saved_or_applied_jobs,
    })))
}

fn serialize_job(job: &JobRecord) -> serde_json::Value {
    let mut val = serde_json::to_value(job).unwrap_or_else(|_| json!({}));
    let dossier = job.dossier();
    let fit = job.fit();
    let screening = job.screening();
    let missing = dossier.as_ref().map(|d| d.missing()).unwrap_or_default();
    let verdict_kind = fit.as_ref().map(|f| f.verdict_kind());

    if let Some(obj) = val.as_object_mut() {
        // Embeddings são insumos internos de ranking; enviá-los ao navegador
        // adiciona dezenas de KB por vaga sem alimentar nenhum fluxo da UI.
        obj.remove("embedding_json");
        // Os objetos analisados abaixo são a representação consumida pela UI;
        // manter também suas strings JSON duplica o conteúdo na resposta.
        obj.remove("dossier_json");
        obj.remove("fit_json");
        obj.remove("screening_json");
        obj.insert("detail_loaded".to_string(), serde_json::Value::Bool(true));
        obj.insert(
            "dossier".to_string(),
            serde_json::to_value(&dossier).unwrap_or(serde_json::Value::Null),
        );
        obj.insert(
            "fit".to_string(),
            serde_json::to_value(&fit).unwrap_or(serde_json::Value::Null),
        );
        obj.insert(
            "screening".to_string(),
            serde_json::to_value(&screening).unwrap_or(serde_json::Value::Null),
        );
        obj.insert(
            "verdict_kind".to_string(),
            verdict_kind
                .map(|v| serde_json::Value::String(v.to_string()))
                .unwrap_or(serde_json::Value::Null),
        );
        obj.insert(
            "missing".to_string(),
            serde_json::to_value(&missing).unwrap_or_else(|_| json!([])),
        );
    }
    val
}

fn serialize_job_summary(job: &JobRecord) -> serde_json::Value {
    let mut val = serialize_job(job);
    if let Some(obj) = val.as_object_mut() {
        for field in [
            "raw_description",
            "match_analysis_md",
            "dossier",
            "fit",
            "screening",
        ] {
            obj.remove(field);
        }
        obj.insert("detail_loaded".to_string(), serde_json::Value::Bool(false));
    }
    val
}

pub async fn api_get_job(
    State(state): State<Arc<WebState>>,
    Path(id): Path<i64>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let job = state
        .db
        .get_job_by_id(id)
        .await
        .map_err(internal_err)?
        .ok_or_else(|| (StatusCode::NOT_FOUND, format!("Vaga #{} não encontrada", id)))?;
    Ok(Json(serialize_job(&job)))
}

pub async fn api_list_jobs(
    State(state): State<Arc<WebState>>,
    Query(params): Query<JobsQueryParams>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let filter = JobFilter {
        query: params.q,
        work_model: params.work_model,
        seniority: params.seniority,
        shift: params.shift,
        status: params.status,
        source: params.source,
        screening_verdict: params.screening_verdict,
        company_id: params.company_id,
        only_enriched: false,
    };

    let jobs = state.db.list_jobs(&filter).await.map_err(internal_err)?;
    let payload: Vec<serde_json::Value> = jobs.iter().map(serialize_job_summary).collect();
    Ok(Json(serde_json::Value::Array(payload)))
}

/// Processa até 1 lote (padrão 5 vagas) por requisição com rastreamento de execução e telemetria IA
async fn run_single_enrichment_batch(
    state: &WebState,
    batch_size: usize,
    exec_handle: Option<&ExecutionHandle>,
) -> Result<(usize, usize)> {
    let api_key = state.cfg.require_gemini_key()?;
    let gemini = GeminiClient::new(api_key)?.with_db(state.db.clone());
    let effective_batch = batch_size.clamp(1, 5);

    let unenriched = state.db.get_unenriched_jobs(effective_batch as i64).await?;
    if unenriched.is_empty() {
        if let Some(h) = exec_handle {
            h.emit_info(
                "enrichment",
                Some("gemini"),
                "Nenhuma vaga elegível pendente de enriquecimento.",
            )
            .await;
        }
        return Ok((0, 0));
    }

    if let Some(h) = exec_handle {
        h.emit_info(
            "enrichment",
            Some("gemini"),
            &format!(
                "Iniciando extração em lote para {} vagas pendentes",
                unenriched.len()
            ),
        )
        .await;
        let _ = h.update_progress(20.0, Some("gemini_extraction"), 0, 0).await;
    }

    let extract_stage = if let Some(h) = exec_handle {
        Some(h.start_stage("gemini_extraction").await?)
    } else {
        None
    };

    let raw_batch: Vec<_> = unenriched.iter().map(|j| j.to_raw_listing()).collect();
    let extracted_list = match gemini.extract_jobs_batch(&raw_batch).await {
        Ok(list) => {
            if let Some(ref stage) = extract_stage {
                stage
                    .complete(raw_batch.len() as i64, list.len() as i64)
                    .await
                    .ok();
            }
            list
        }
        Err(err) => {
            warn!("Erro na extração em lote via Web API: {}", err);
            if let Some(ref stage) = extract_stage {
                stage.fail(&err.to_string()).await.ok();
            }
            if let Some(h) = exec_handle {
                h.emit_error(
                    "gemini_extraction",
                    Some("gemini"),
                    &format!("Erro na extração: {}", err),
                    None,
                )
                .await;
            }
            return Err(err);
        }
    };

    if let Some(h) = exec_handle {
        let _ = h
            .update_progress(60.0, Some("gemini_embedding"), unenriched.len() as i64, 0)
            .await;
    }

    let embed_stage = if let Some(h) = exec_handle {
        Some(h.start_stage("gemini_embedding").await?)
    } else {
        None
    };

    let mut enriched_count = 0usize;

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
        meta.dossier
            .verify_evidence(job_record.raw_description.as_deref().unwrap_or(""));

        let embedding_text = job_record.build_embedding_text(&meta);
        let embedding_opt = gemini.generate_embedding(&embedding_text).await.ok();

        state
            .db
            .update_job_enrichment(job_record.id, &meta, embedding_opt.as_deref())
            .await?;
        enriched_count += 1;
    }

    if let Some(ref stage) = embed_stage {
        stage
            .complete(unenriched.len() as i64, enriched_count as i64)
            .await
            .ok();
    }

    let remaining = state.db.get_unenriched_jobs(100).await?.len();

    if let Some(h) = exec_handle {
        let _ = h
            .update_progress(100.0, Some("conclusao"), enriched_count as i64, 0)
            .await;
        h.emit_info(
            "conclusao",
            Some("gemini"),
            &format!(
                "Lote concluído: {} vagas enriquecidas (restam {} pendentes)",
                enriched_count, remaining
            ),
        )
        .await;
        h.record_metric("enriched_count", enriched_count as f64, "count")
            .await
            .ok();
        h.record_metric("remaining_unenriched", remaining as f64, "count")
            .await
            .ok();
    }

    Ok((enriched_count, remaining))
}

async fn api_collect(
    State(state): State<Arc<WebState>>,
    Json(payload): Json<CollectPayload>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let meta = json!({
        "query": payload.query,
        "location": payload.location,
        "limit": payload.limit,
        "remote": payload.remote,
        "enrich": payload.enrich,
        "sources": payload.sources,
    });

    let exec_handle = state
        .engine
        .start_execution(ExecutionType::JobSearch, "MANUAL_WEB", Some(meta))
        .await
        .map_err(internal_err)?;

    let rep = match collectors::collect_with_sources(
        &state.cfg,
        &state.db,
        &payload.query,
        payload.location.as_deref(),
        payload.limit,
        payload.remote,
        payload.sources.as_deref(),
        Some(&exec_handle),
    )
    .await
    {
        Ok(r) => {
            let _ = exec_handle
                .finish_success(
                    r.newly_inserted as i64,
                    0,
                    Some("Busca de vagas multi-fonte concluída com sucesso"),
                )
                .await;
            r
        }
        Err(err) => {
            let _ = exec_handle.finish_failure(&err.to_string()).await;
            return Err(internal_err(err));
        }
    };

    Ok(Json(json!({
        "execution_id": exec_handle.id(),
        "gupy_count": rep.gupy_count,
        "jobspy_count": rep.jobspy_count,
        "linkedin_count": rep.linkedin_count,
        "indeed_count": rep.indeed_count,
        "glassdoor_count": rep.glassdoor_count,
        "vagas_com_count": rep.vagas_com_count,
        "ats_count": rep.ats_count,
        "total_fetched": rep.total_fetched,
        "newly_inserted": rep.newly_inserted,
        "duplicates_skipped": rep.duplicates_skipped,
        "warnings": rep.warnings,
        "source_diagnostics": rep.source_diagnostics,
        "should_enrich": payload.enrich && rep.newly_inserted > 0,
    })))
}

async fn api_enrich(
    State(state): State<Arc<WebState>>,
    body: Bytes,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let payload: EnrichPayload = if body.is_empty() {
        EnrichPayload { batch_size: 5 }
    } else {
        serde_json::from_slice(&body).unwrap_or(EnrichPayload { batch_size: 5 })
    };

    let meta = json!({ "batch_size": payload.batch_size });
    let exec_handle = state
        .engine
        .start_execution(ExecutionType::AiEnrichment, "MANUAL_WEB", Some(meta))
        .await
        .map_err(internal_err)?;

    let (enriched_count, remaining) = match run_single_enrichment_batch(
        &state,
        payload.batch_size,
        Some(&exec_handle),
    )
    .await
    {
        Ok(r) => {
            let _ = exec_handle
                .finish_success(
                    r.0 as i64,
                    0,
                    Some(&format!("Enriquecimento de lote finalizado: {} vagas", r.0)),
                )
                .await;
            r
        }
        Err(err) => {
            let _ = exec_handle.finish_failure(&err.to_string()).await;
            return Err(internal_err(err));
        }
    };

    Ok(Json(json!({
        "execution_id": exec_handle.id(),
        "enriched_count": enriched_count,
        "remaining": remaining,
    })))
}

async fn api_rank(
    State(state): State<Arc<WebState>>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let exec_handle = state
        .engine
        .start_execution(ExecutionType::JobScreening, "MANUAL_WEB", None)
        .await
        .map_err(internal_err)?;

    // Primeiro garante que todas as vagas estejam com a triagem determinística atualizada
    let _ = state.db.screen_all_jobs().await;

    let api_key = state.cfg.require_gemini_key().map_err(internal_err)?;
    let gemini = GeminiClient::new(api_key)
        .map_err(internal_err)?
        .with_db(state.db.clone());

    let resume_md = fs::read_to_string(&state.cfg.resume_path).map_err(internal_err)?;
    let resume_embedding = gemini
        .generate_embedding(&resume_md)
        .await
        .map_err(internal_err)?;

    let ranked = state
        .db
        .rank_jobs_by_embedding(&resume_embedding, 200)
        .await
        .map_err(internal_err)?;

    let count = ranked.len();
    let _ = exec_handle
        .finish_success(
            count as i64,
            0,
            Some(&format!(
                "Triagem + Ranking Semântico concluídos: {} vagas ranqueadas",
                count
            )),
        )
        .await;

    let items: Vec<_> = ranked
        .into_iter()
        .map(|(job, sim)| {
            json!({
                "job_id": job.id,
                "similarity": sim,
            })
        })
        .collect();

    Ok(Json(json!({ "execution_id": exec_handle.id(), "ranked": items })))
}

async fn api_screen_job(
    State(state): State<Arc<WebState>>,
    Path(job_id): Path<i64>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let res = state
        .db
        .screen_job_by_id(job_id)
        .await
        .map_err(internal_err)?
        .ok_or_else(|| (StatusCode::NOT_FOUND, format!("Vaga #{} não encontrada", job_id)))?;
    Ok(Json(serde_json::to_value(res).unwrap_or(json!({}))))
}

async fn api_screen_all_jobs(
    State(state): State<Arc<WebState>>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let exec_handle = state
        .engine
        .start_execution(ExecutionType::JobScreening, "MANUAL_WEB", None)
        .await
        .map_err(internal_err)?;

    let count = state.db.screen_all_jobs().await.map_err(internal_err)?;
    let _ = exec_handle
        .finish_success(
            count as i64,
            0,
            Some(&format!(
                "Triagem determinística executada em {} vagas",
                count
            )),
        )
        .await;

    Ok(Json(json!({
        "execution_id": exec_handle.id(),
        "screened_count": count,
        "ok": true
    })))
}

async fn api_consolidate_duplicates(
    State(state): State<Arc<WebState>>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let (consolidated, remaining) = state
        .db
        .consolidate_duplicate_jobs()
        .await
        .map_err(internal_err)?;

    Ok(Json(json!({
        "ok": true,
        "consolidated_count": consolidated,
        "remaining_jobs": remaining,
    })))
}

async fn api_get_job_sources(
    State(state): State<Arc<WebState>>,
    Path(job_id): Path<i64>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let sources = state
        .db
        .get_job_sources(job_id)
        .await
        .map_err(internal_err)?;
    Ok(Json(serde_json::to_value(sources).unwrap_or(json!([]))))
}

async fn api_job_match(
    State(state): State<Arc<WebState>>,
    Path(job_id): Path<i64>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let meta = json!({ "job_id": job_id });
    let exec_handle = state
        .engine
        .start_execution(ExecutionType::MatchAnalysis, "MANUAL_WEB", Some(meta))
        .await
        .map_err(internal_err)?;

    let api_key = state.cfg.require_gemini_key().map_err(internal_err)?;
    let gemini = GeminiClient::new(api_key)
        .map_err(internal_err)?
        .with_db(state.db.clone());

    let job = state
        .db
        .get_job_by_id(job_id)
        .await
        .map_err(internal_err)?
        .ok_or_else(|| (StatusCode::NOT_FOUND, format!("Vaga #{} não encontrada", job_id)))?;

    exec_handle
        .emit_info(
            "match_analysis",
            Some("gemini"),
            &format!(
                "Iniciando análise profunda de Fit ATS para #{} ({})",
                job.id,
                job.title_str()
            ),
        )
        .await;

    let resume_md = fs::read_to_string(&state.cfg.resume_path).map_err(internal_err)?;
    let profile = state.db.get_candidate_profile().await.unwrap_or_default();
    let analysis = match gemini
        .analyze_job_match_with_profile(&resume_md, &job, &profile)
        .await
    {
        Ok(a) => a,
        Err(err) => {
            let _ = exec_handle.finish_failure(&err.to_string()).await;
            return Err(internal_err(err));
        }
    };

    let verdict_kind = analysis.verdict_kind();
    state
        .db
        .update_job_match(job.id, &analysis)
        .await
        .map_err(internal_err)?;

    let updated_job = state
        .db
        .get_job_by_id(job.id)
        .await
        .map_err(internal_err)?
        .map(|j| serialize_job(&j));

    let _ = exec_handle
        .finish_success(
            1,
            0,
            Some(&format!(
                "Match calculado: {:.0}% ({})",
                analysis.match_score, verdict_kind
            )),
        )
        .await;

    Ok(Json(json!({
        "execution_id": exec_handle.id(),
        "job_id": job.id,
        "verdict_kind": verdict_kind,
        "analysis": analysis,
        "fit": analysis,
        "job": updated_job,
    })))
}

async fn api_update_status(
    State(state): State<Arc<WebState>>,
    Path(job_id): Path<i64>,
    Json(payload): Json<StatusUpdatePayload>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    state
        .db
        .update_job_status(job_id, &payload.status)
        .await
        .map_err(internal_err)?;

    Ok(Json(json!({ "id": job_id, "status": payload.status, "ok": true })))
}

// ----------------- Sprint 09: Coletores -----------------

async fn api_list_collectors(
    State(state): State<Arc<WebState>>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let list = state.db.list_collectors().await.map_err(internal_err)?;
    Ok(Json(serde_json::to_value(list).unwrap_or(json!([]))))
}

async fn api_toggle_collector(
    State(state): State<Arc<WebState>>,
    Path(source_id): Path<String>,
    Json(payload): Json<ToggleSavedSearchPayload>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    state
        .db
        .toggle_collector(&source_id, payload.enabled)
        .await
        .map_err(internal_err)?;
    Ok(Json(json!({ "source_id": source_id, "enabled": payload.enabled, "ok": true })))
}

// ----------------- Sprint 11: Company Intelligence -----------------

async fn api_list_companies(
    State(state): State<Arc<WebState>>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let list = state.db.list_companies(false).await.map_err(internal_err)?;
    Ok(Json(serde_json::to_value(list).unwrap_or(json!([]))))
}

async fn api_get_company_details(
    State(state): State<Arc<WebState>>,
    Path(company_id): Path<i64>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let detail = state
        .db
        .get_company_details(company_id)
        .await
        .map_err(internal_err)?
        .ok_or_else(|| (StatusCode::NOT_FOUND, format!("Empresa #{} não encontrada", company_id)))?;
    Ok(Json(serde_json::to_value(detail).unwrap_or(json!({}))))
}

async fn api_upsert_company(
    State(state): State<Arc<WebState>>,
    Json(payload): Json<UpsertCompanyPayload>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let comp_id = state
        .db
        .upsert_company(
            &payload.official_name,
            &payload.website,
            &payload.segment,
            &payload.location,
            &payload.description,
            payload.is_monitored,
            payload.is_preferred,
            payload.is_excluded,
        )
        .await
        .map_err(internal_err)?;

    if let (Some(ats), Some(portal)) = (&payload.ats_provider, &payload.portal_id) {
        if !ats.trim().is_empty() && !portal.trim().is_empty() {
            let career_url = payload
                .career_url
                .clone()
                .unwrap_or_else(|| payload.website.clone());
            let _ = state
                .db
                .add_company_source(comp_id, "ATS_BOARD", &career_url, ats, portal)
                .await;
        }
    }

    // Reavalia triagem determinística caso preferências de empresa tenham mudado
    let _ = state.db.screen_all_jobs().await;

    Ok(Json(json!({ "id": comp_id, "ok": true })))
}

async fn api_add_company_alias(
    State(state): State<Arc<WebState>>,
    Path(company_id): Path<i64>,
    Json(payload): Json<AddAliasPayload>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let id = state
        .db
        .add_company_alias(company_id, &payload.alias_name)
        .await
        .map_err(internal_err)?;
    Ok(Json(json!({ "id": id, "ok": true })))
}

async fn api_add_company_source(
    State(state): State<Arc<WebState>>,
    Path(company_id): Path<i64>,
    Json(payload): Json<AddCompanySourcePayload>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let id = state
        .db
        .add_company_source(
            company_id,
            &payload.source_type,
            &payload.career_url,
            &payload.ats_provider,
            &payload.portal_id,
        )
        .await
        .map_err(internal_err)?;
    Ok(Json(json!({ "id": id, "ok": true })))
}

async fn api_scan_company(
    State(state): State<Arc<WebState>>,
    Path(company_id): Path<i64>,
    body: Bytes,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let payload: CompanyScanPayload = if body.is_empty() {
        CompanyScanPayload {
            query: None,
            limit: 30,
        }
    } else {
        serde_json::from_slice(&body).unwrap_or(CompanyScanPayload {
            query: None,
            limit: 30,
        })
    };

    let meta = json!({
        "company_id": company_id,
        "query": payload.query,
        "limit": payload.limit,
    });

    let exec_handle = state
        .engine
        .start_execution(ExecutionType::CompanySearch, "MANUAL_WEB", Some(meta))
        .await
        .map_err(internal_err)?;

    match state
        .db
        .scan_company_opportunities(
            company_id,
            payload.query.as_deref(),
            payload.limit,
            Some(&exec_handle),
        )
        .await
    {
        Ok(rep) => {
            let _ = exec_handle
                .finish_success(
                    rep.newly_inserted as i64,
                    0,
                    Some(&format!(
                        "Varredura da empresa '{}' concluída: {} novas vagas",
                        rep.company_name, rep.newly_inserted
                    )),
                )
                .await;
            Ok(Json(serde_json::to_value(rep).unwrap_or(json!({}))))
        }
        Err(err) => {
            let _ = exec_handle.finish_failure(&err.to_string()).await;
            Err(internal_err(err))
        }
    }
}

// ----------------- Sprint 13: Telemetria de IA -----------------

async fn api_ai_telemetry(
    State(state): State<Arc<WebState>>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let summary = state
        .db
        .get_ai_telemetry_summary()
        .await
        .map_err(internal_err)?;
    Ok(Json(serde_json::to_value(summary).unwrap_or(json!({}))))
}

// ----------------- Currículo, Perfil e Automação -----------------

async fn api_get_resume(
    State(state): State<Arc<WebState>>,
) -> Result<Json<ResumePayload>, (StatusCode, String)> {
    let content = fs::read_to_string(&state.cfg.resume_path).unwrap_or_default();
    Ok(Json(ResumePayload { content }))
}

async fn api_put_resume(
    State(state): State<Arc<WebState>>,
    Json(payload): Json<ResumePayload>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    fs::write(&state.cfg.resume_path, &payload.content).map_err(|err| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Erro ao salvar currículo: {}", err),
        )
    })?;
    Ok(Json(json!({ "ok": true })))
}

#[derive(Debug, Serialize)]
pub struct ResumeUploadPdfResponse {
    pub content: String,
    pub suggested_name: Option<String>,
    pub suggested_location: Option<String>,
    pub suggested_role: Option<String>,
    pub skills: Vec<String>,
    pub char_count: usize,
    pub ai_structured: bool,
}

fn clean_pdf_text(raw: &str) -> String {
    let mut cleaned = String::new();
    let mut consecutive_newlines = 0;

    for line in raw.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            consecutive_newlines += 1;
            if consecutive_newlines <= 2 {
                cleaned.push('\n');
            }
        } else {
            consecutive_newlines = 0;
            if !cleaned.is_empty() && !cleaned.ends_with('\n') {
                cleaned.push('\n');
            }
            cleaned.push_str(trimmed);
        }
    }

    cleaned.trim().to_string()
}

async fn api_upload_resume_pdf(
    State(state): State<Arc<WebState>>,
    body: Bytes,
) -> Result<Json<ResumeUploadPdfResponse>, (StatusCode, String)> {
    if body.is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            "Arquivo PDF vazio recebido.".to_string(),
        ));
    }

    let extracted = pdf_extract::extract_text_from_mem(&body).map_err(|e| {
        (
            StatusCode::BAD_REQUEST,
            format!("Não foi possível extrair texto do PDF: {}", e),
        )
    })?;

    let cleaned = clean_pdf_text(&extracted);
    if cleaned.trim().is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            "O arquivo PDF não contém texto legível ou extraível (pode ser um documento digitalizado apenas com imagens).".to_string(),
        ));
    }

    let char_count = cleaned.chars().count();

    // Se houver chave do Gemini configurada, tentar estruturação inteligente com IA
    if let Ok(key) = state.cfg.require_gemini_key() {
        if let Ok(gemini) = GeminiClient::new(key) {
            let gemini = gemini.with_db(state.db.clone());
            match gemini.structure_resume_from_text(&cleaned).await {
                Ok(structured) => {
                    return Ok(Json(ResumeUploadPdfResponse {
                        content: structured.markdown,
                        suggested_name: structured.suggested_name,
                        suggested_location: structured.suggested_location,
                        suggested_role: structured.suggested_role,
                        skills: structured.skills,
                        char_count,
                        ai_structured: true,
                    }));
                }
                Err(err) => {
                    warn!("Falha ao estruturar currículo via Gemini, usando fallback offline: {:#}", err);
                }
            }
        }
    }

    // Fallback offline caso o Gemini não esteja ativo ou falhe
    let fallback_markdown = format!(
        "# Currículo Profissional (Importado de PDF)\n\n{}\n",
        cleaned
    );

    Ok(Json(ResumeUploadPdfResponse {
        content: fallback_markdown,
        suggested_name: None,
        suggested_location: None,
        suggested_role: None,
        skills: Vec::new(),
        char_count,
        ai_structured: false,
    }))
}

async fn api_get_profile(
    State(state): State<Arc<WebState>>,
) -> Result<Json<CandidateProfile>, (StatusCode, String)> {
    let p = state
        .db
        .get_candidate_profile()
        .await
        .map_err(internal_err)?;
    Ok(Json(p))
}

async fn api_put_profile(
    State(state): State<Arc<WebState>>,
    Json(payload): Json<CandidateProfile>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    state
        .db
        .update_candidate_profile(&payload)
        .await
        .map_err(internal_err)?;
    let _ = state.db.screen_all_jobs().await;
    Ok(Json(json!({ "ok": true })))
}

async fn api_get_automation(
    State(state): State<Arc<WebState>>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let searches = state.db.list_saved_searches().await.map_err(internal_err)?;
    let status_guard = state.auto_status.read().await;
    Ok(Json(json!({
        "searches": searches,
        "status": *status_guard,
    })))
}

async fn api_create_saved_search(
    State(state): State<Arc<WebState>>,
    Json(payload): Json<NewSavedSearchPayload>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let id = state
        .db
        .insert_saved_search(
            &payload.query,
            &payload.location,
            payload.remote,
            payload.interval_minutes,
        )
        .await
        .map_err(internal_err)?;
    Ok(Json(json!({ "id": id, "ok": true })))
}

async fn api_toggle_saved_search(
    State(state): State<Arc<WebState>>,
    Path(id): Path<i64>,
    Json(payload): Json<ToggleSavedSearchPayload>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    state
        .db
        .toggle_saved_search(id, payload.enabled)
        .await
        .map_err(internal_err)?;
    Ok(Json(json!({ "ok": true })))
}

async fn api_delete_saved_search(
    State(state): State<Arc<WebState>>,
    Path(id): Path<i64>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    state
        .db
        .delete_saved_search(id)
        .await
        .map_err(internal_err)?;
    Ok(Json(json!({ "ok": true })))
}

async fn api_run_automation_now(
    State(state): State<Arc<WebState>>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let summary = automation::run_automation_tick(
        &state.cfg,
        &state.db,
        &state.engine,
        &state.auto_status,
        true,
    )
    .await
    .map_err(internal_err)?;

    Ok(Json(json!({
        "ok": true,
        "summary": summary,
    })))
}

// ----------------- Observabilidade e Execution Engine -----------------

pub async fn api_list_executions(
    State(state): State<Arc<WebState>>,
    Query(params): Query<ExecutionsQueryParams>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let limit = params.limit.unwrap_or(30);
    let offset = params.offset.unwrap_or(0);
    let list = state
        .engine
        .list_executions(
            limit,
            offset,
            params.r#type.as_deref(),
            params.status.as_deref(),
        )
        .await
        .map_err(internal_err)?;
    Ok(Json(serde_json::to_value(list).unwrap_or(json!([]))))
}

pub async fn api_get_execution(
    State(state): State<Arc<WebState>>,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let detail = state
        .engine
        .get_execution_detail(&id)
        .await
        .map_err(internal_err)?
        .ok_or_else(|| (StatusCode::NOT_FOUND, format!("Execução '{}' não encontrada", id)))?;
    Ok(Json(serde_json::to_value(detail).unwrap_or(json!({}))))
}

async fn api_cancel_execution(
    State(state): State<Arc<WebState>>,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let cancelled = state
        .engine
        .cancel_execution(&id)
        .await
        .map_err(internal_err)?;
    Ok(Json(json!({ "ok": true, "cancelled": cancelled })))
}

async fn api_get_executions_summary(
    State(state): State<Arc<WebState>>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let summary = state
        .engine
        .get_executions_summary()
        .await
        .map_err(internal_err)?;
    Ok(Json(serde_json::to_value(summary).unwrap_or(json!({}))))
}

async fn api_retry_execution(
    State(state): State<Arc<WebState>>,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let orig = state
        .engine
        .get_execution(&id)
        .await
        .map_err(internal_err)?
        .ok_or_else(|| (StatusCode::NOT_FOUND, format!("Execução '{}' não encontrada", id)))?;

    let meta: serde_json::Value = orig
        .metadata_json
        .as_deref()
        .and_then(|m| serde_json::from_str(m).ok())
        .unwrap_or(json!({}));

    match orig.execution_type.as_str() {
        "JOB_SEARCH" => {
            let query = meta.get("query").and_then(|v| v.as_str()).unwrap_or("TI").to_string();
            let location = meta.get("location").and_then(|v| v.as_str()).map(|s| s.to_string());
            let limit = meta.get("limit").and_then(|v| v.as_u64()).unwrap_or(20) as usize;
            let remote = meta.get("remote").and_then(|v| v.as_bool()).unwrap_or(false);
            let sources = meta.get("sources").and_then(|v| v.as_array()).map(|arr| {
                arr.iter().filter_map(|s| s.as_str().map(|str| str.to_string())).collect::<Vec<_>>()
            });

            let retry_meta = json!({
                "retry_of": id,
                "query": query,
                "location": location,
                "limit": limit,
                "remote": remote,
                "sources": sources,
            });

            let new_handle = state
                .engine
                .start_execution(ExecutionType::JobSearch, "RETRY", Some(retry_meta))
                .await
                .map_err(internal_err)?;

            let new_id = new_handle.id().to_string();
            let state_clone = state.clone();

            tokio::spawn(async move {
                let rep = collectors::collect_with_sources(
                    &state_clone.cfg,
                    &state_clone.db,
                    &query,
                    location.as_deref(),
                    limit,
                    remote,
                    sources.as_deref(),
                    Some(&new_handle),
                )
                .await;

                match rep {
                    Ok(r) => {
                        let _ = new_handle
                            .finish_success(
                                r.newly_inserted as i64,
                                0,
                                Some("Reexecução de busca concluída com sucesso"),
                            )
                            .await;
                    }
                    Err(err) => {
                        let _ = new_handle.finish_failure(&err.to_string()).await;
                    }
                }
            });

            Ok(Json(json!({ "ok": true, "new_execution_id": new_id })))
        }
        "COMPANY_SEARCH" | "COMPANY_MONITORING" => {
            let comp_id = meta.get("company_id").and_then(|v| v.as_i64()).unwrap_or(1);
            let query = meta.get("query").and_then(|v| v.as_str()).map(|s| s.to_string());
            let limit = meta.get("limit").and_then(|v| v.as_u64()).unwrap_or(30) as usize;

            let retry_meta = json!({
                "retry_of": id,
                "company_id": comp_id,
                "query": query,
                "limit": limit,
            });

            let new_handle = state
                .engine
                .start_execution(ExecutionType::CompanyMonitoring, "RETRY", Some(retry_meta))
                .await
                .map_err(internal_err)?;

            let new_id = new_handle.id().to_string();
            let state_clone = state.clone();

            tokio::spawn(async move {
                match state_clone
                    .db
                    .scan_company_opportunities(comp_id, query.as_deref(), limit, Some(&new_handle))
                    .await
                {
                    Ok(rep) => {
                        let _ = new_handle
                            .finish_success(
                                rep.newly_inserted as i64,
                                0,
                                Some(&format!(
                                    "Reexecução da empresa '{}' finalizada ({} vagas)",
                                    rep.company_name, rep.newly_inserted
                                )),
                            )
                            .await;
                    }
                    Err(err) => {
                        let _ = new_handle.finish_failure(&err.to_string()).await;
                    }
                }
            });

            Ok(Json(json!({ "ok": true, "new_execution_id": new_id })))
        }
        "AUTOMATION" => {
            let state_clone = state.clone();
            tokio::spawn(async move {
                let _ = automation::run_automation_tick(
                    &state_clone.cfg,
                    &state_clone.db,
                    &state_clone.engine,
                    &state_clone.auto_status,
                    true,
                )
                .await;
            });
            Ok(Json(json!({ "ok": true, "message": "Ciclo de automação disparado em segundo plano" })))
        }
        _ => {
            Err((
                StatusCode::BAD_REQUEST,
                format!("Reexecução não suportada para tipo '{}'", orig.execution_type),
            ))
        }
    }
}

async fn api_executions_stream(
    State(state): State<Arc<WebState>>,
) -> Sse<impl tokio_stream::Stream<Item = Result<SseEvent, std::convert::Infallible>>> {
    let rx = state.engine.subscribe();
    let stream = BroadcastStream::new(rx).filter_map(|msg| match msg {
        Ok(event_msg) => {
            let json_str = serde_json::to_string(&event_msg).unwrap_or_default();
            Some(Ok(SseEvent::default().event("execution_event").data(json_str)))
        }
        Err(_) => None,
    });

    Sse::new(stream).keep_alive(KeepAlive::default())
}

async fn api_execution_id_stream(
    State(state): State<Arc<WebState>>,
    Path(target_id): Path<String>,
) -> Sse<impl tokio_stream::Stream<Item = Result<SseEvent, std::convert::Infallible>>> {
    let rx = state.engine.subscribe();
    let stream = BroadcastStream::new(rx).filter_map(move |msg| match msg {
        Ok(event_msg) => {
            let matches_id = match &event_msg {
                ExecutionEventMessage::ExecutionCreated(e) => e.id == target_id,
                ExecutionEventMessage::ExecutionUpdated(e) => e.id == target_id,
                ExecutionEventMessage::StageStarted(s) => s.execution_id == target_id,
                ExecutionEventMessage::StageCompleted(s) => s.execution_id == target_id,
                ExecutionEventMessage::EventLogged(e) => e.execution_id == target_id,
                ExecutionEventMessage::MetricRecorded(m) => m.execution_id == target_id,
            };
            if matches_id {
                let json_str = serde_json::to_string(&event_msg).unwrap_or_default();
                Some(Ok(SseEvent::default().event("execution_event").data(json_str)))
            } else {
                None
            }
        }
        Err(_) => None,
    });

    Sse::new(stream).keep_alive(KeepAlive::default())
}
