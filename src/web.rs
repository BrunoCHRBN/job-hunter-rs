use std::fs;
use std::net::SocketAddr;
use std::sync::Arc;

use anyhow::{Context, Result};
use axum::body::Bytes;
use axum::extract::{Path, Query, State};
use axum::http::{header, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use serde_json::json;
use tracing::{info, warn};

use crate::collectors;
use crate::config::AppConfig;
use crate::db::Database;
use crate::gemini::GeminiClient;
use crate::models::{JobFilter, JobRecord};

const INDEX_HTML: &str = include_str!("../static/index.html");
const STYLES_CSS: &str = include_str!("../static/styles.css");
const APP_JS: &str = include_str!("../static/app.js");

#[derive(Clone)]
pub struct WebState {
    pub cfg: AppConfig,
    pub db: Database,
}

#[derive(Debug, Deserialize, Default)]
pub struct JobsQueryParams {
    pub work_model: Option<String>,
    pub seniority: Option<String>,
    pub shift: Option<String>,
    pub status: Option<String>,
    pub q: Option<String>,
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

pub async fn run_web_server(cfg: AppConfig, db: Database, port: u16) -> Result<()> {
    let state = Arc::new(WebState { cfg, db });

    let app = Router::new()
        .route("/", get(serve_index))
        .route("/index.html", get(serve_index))
        .route("/styles.css", get(serve_css))
        .route("/static/styles.css", get(serve_css))
        .route("/app.js", get(serve_js))
        .route("/static/app.js", get(serve_js))
        .route("/api/stats", get(api_stats))
        .route("/api/jobs", get(api_list_jobs))
        .route("/api/collect", post(api_collect))
        .route("/api/search", post(api_collect))
        .route("/api/enrich", post(api_enrich))
        .route("/api/rank", post(api_rank))
        .route("/api/jobs/:id/match", post(api_job_match))
        .route(
            "/api/jobs/:id/status",
            post(api_update_status).patch(api_update_status),
        )
        .route("/api/resume", get(api_get_resume).put(api_put_resume))
        .with_state(state);

    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    info!("🌐 Servidor Web iniciado em http://127.0.0.1:{}", port);
    println!("🌐 Bancada Web Job Hunter RS ativa em: http://localhost:{}", port);

    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .with_context(|| format!("Falha ao fazer bind na porta {}", port))?;

    axum::serve(listener, app)
        .await
        .context("Erro durante execução do servidor HTTP Axum")?;

    Ok(())
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
    let missing = dossier
        .as_ref()
        .map(|d| d.missing())
        .unwrap_or_default();
    let verdict_kind = fit.as_ref().map(|f| f.verdict_kind());

    if let Some(obj) = val.as_object_mut() {
        obj.insert(
            "dossier".to_string(),
            serde_json::to_value(&dossier).unwrap_or(serde_json::Value::Null),
        );
        obj.insert(
            "fit".to_string(),
            serde_json::to_value(&fit).unwrap_or(serde_json::Value::Null),
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

async fn api_list_jobs(
    State(state): State<Arc<WebState>>,
    Query(params): Query<JobsQueryParams>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let filter = JobFilter {
        query: params.q,
        work_model: params.work_model,
        seniority: params.seniority,
        shift: params.shift,
        status: params.status,
        only_enriched: false,
    };

    let jobs = state.db.list_jobs(&filter).await.map_err(internal_err)?;
    let payload: Vec<serde_json::Value> = jobs.iter().map(serialize_job).collect();
    Ok(Json(serde_json::Value::Array(payload)))
}

/// Processa até 1 lote (padrão 5 vagas) por requisição para responder rápido (< 5s)
/// e evitar timeouts HTTP no navegador. Retorna `(enriched_count, remaining_unenriched)`.
async fn run_single_enrichment_batch(state: &WebState, batch_size: usize) -> Result<(usize, usize)> {
    let api_key = state.cfg.require_gemini_key()?;
    let gemini = GeminiClient::new(api_key)?;
    let effective_batch = batch_size.clamp(1, 5);

    let unenriched = state
        .db
        .get_unenriched_jobs(effective_batch as i64)
        .await?;
    if unenriched.is_empty() {
        return Ok((0, 0));
    }

    let raw_batch: Vec<_> = unenriched.iter().map(|j| j.to_raw_listing()).collect();
    let extracted_list = match gemini.extract_jobs_batch(&raw_batch).await {
        Ok(list) => list,
        Err(err) => {
            warn!("Erro na extração em lote via Web API: {}", err);
            return Err(err);
        }
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

    let remaining = state.db.get_unenriched_jobs(100).await?.len();
    Ok((enriched_count, remaining))
}

async fn api_collect(
    State(state): State<Arc<WebState>>,
    Json(payload): Json<CollectPayload>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let report = collectors::collect_all(
        &state.cfg,
        &state.db,
        &payload.query,
        payload.location.as_deref(),
        payload.limit,
        payload.remote,
    )
    .await
    .map_err(internal_err)?;

    Ok(Json(json!({
        "gupy_count": report.gupy_count,
        "jobspy_count": report.jobspy_count,
        "total_fetched": report.total_fetched,
        "newly_inserted": report.newly_inserted,
        "duplicates_skipped": report.duplicates_skipped,
        "should_enrich": payload.enrich && report.newly_inserted > 0,
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

    let (enriched_count, remaining) = run_single_enrichment_batch(&state, payload.batch_size)
        .await
        .map_err(internal_err)?;

    Ok(Json(json!({
        "enriched_count": enriched_count,
        "remaining": remaining,
    })))
}

async fn api_rank(
    State(state): State<Arc<WebState>>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let api_key = state.cfg.require_gemini_key().map_err(internal_err)?;
    let gemini = GeminiClient::new(api_key).map_err(internal_err)?;

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

    let items: Vec<_> = ranked
        .into_iter()
        .map(|(job, sim)| {
            json!({
                "job_id": job.id,
                "similarity": sim,
            })
        })
        .collect();

    Ok(Json(json!({ "ranked": items })))
}

async fn api_job_match(
    State(state): State<Arc<WebState>>,
    Path(job_id): Path<i64>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let api_key = state.cfg.require_gemini_key().map_err(internal_err)?;
    let gemini = GeminiClient::new(api_key).map_err(internal_err)?;

    let job = state
        .db
        .get_job_by_id(job_id)
        .await
        .map_err(internal_err)?
        .ok_or_else(|| (StatusCode::NOT_FOUND, format!("Vaga #{} não encontrada", job_id)))?;

    let resume_md = fs::read_to_string(&state.cfg.resume_path).map_err(internal_err)?;
    let analysis = gemini
        .analyze_job_match(&resume_md, &job)
        .await
        .map_err(internal_err)?;

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

    Ok(Json(json!({
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
    Ok(Json(json!({ "ok": true })))
}

async fn api_get_resume(
    State(state): State<Arc<WebState>>,
) -> Result<Json<ResumePayload>, (StatusCode, String)> {
    let content = fs::read_to_string(&state.cfg.resume_path).map_err(internal_err)?;
    Ok(Json(ResumePayload { content }))
}

async fn api_put_resume(
    State(state): State<Arc<WebState>>,
    Json(payload): Json<ResumePayload>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    fs::write(&state.cfg.resume_path, payload.content).map_err(internal_err)?;
    Ok(Json(json!({ "ok": true })))
}
