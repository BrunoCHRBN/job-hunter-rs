use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Instant;

use anyhow::{Context, Result};
use chrono::Utc;
use tokio::sync::{broadcast, RwLock};
use tracing::{debug, error, info, warn};

use sqlx::Row;

use super::models::{
    EventLevel, EventRecord, ExecutionDetail, ExecutionEventMessage, ExecutionRecord,
    ExecutionStatus, ExecutionsSummary, ExecutionType, MetricRecord, StageRecord, StageStatus,
};
use crate::db::Database;

fn generate_id(prefix: &str) -> String {
    let now = Utc::now();
    let seed = format!("{}:{}", now.timestamp_nanos_opt().unwrap_or(0), std::process::id());
    let hash = blake3::hash(seed.as_bytes()).to_hex();
    format!("{}_{}_{}", prefix, now.format("%Y%m%d_%H%M%S"), &hash[..8])
}

fn generate_event_id() -> String {
    let now = Utc::now();
    let seed = format!("evt:{}:{}", now.timestamp_nanos_opt().unwrap_or(0), rand_hint());
    let hash = blake3::hash(seed.as_bytes()).to_hex();
    format!("evt_{}_{}", now.timestamp_micros(), &hash[..6])
}

fn rand_hint() -> u64 {
    static COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
    COUNTER.fetch_add(1, Ordering::Relaxed)
}

pub struct ExecutionEngine {
    db: Database,
    event_tx: broadcast::Sender<ExecutionEventMessage>,
    cancellations: RwLock<HashMap<String, Arc<AtomicBool>>>,
}

impl ExecutionEngine {
    pub fn new(db: Database) -> Arc<Self> {
        let (event_tx, _) = broadcast::channel(1024);
        Arc::new(Self {
            db,
            event_tx,
            cancellations: RwLock::new(HashMap::new()),
        })
    }

    pub fn subscribe(&self) -> broadcast::Receiver<ExecutionEventMessage> {
        self.event_tx.subscribe()
    }

    pub fn db(&self) -> &Database {
        &self.db
    }

    /// Cria e inicia uma nova execução no banco SQLite e no canal em tempo real.
    pub async fn start_execution(
        self: &Arc<Self>,
        exec_type: ExecutionType,
        origin: &str,
        metadata: Option<serde_json::Value>,
    ) -> Result<ExecutionHandle> {
        let id = generate_id("exec");
        let now_str = Utc::now().to_rfc3339();
        let meta_str = metadata.map(|m| m.to_string());
        let cancel_flag = Arc::new(AtomicBool::new(false));

        sqlx::query(
            "INSERT INTO executions (
                id, execution_type, request_origin, status, current_stage,
                progress_pct, processed_count, failed_count, diagnostic_message,
                metadata_json, started_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
        )
        .bind(&id)
        .bind(exec_type.as_str())
        .bind(origin)
        .bind(ExecutionStatus::Running.as_str())
        .bind(Option::<String>::None)
        .bind(0.0)
        .bind(0_i64)
        .bind(0_i64)
        .bind(Option::<String>::None)
        .bind(&meta_str)
        .bind(&now_str)
        .execute(self.db.pool())
        .await
        .with_context(|| format!("Erro ao criar execução {}", id))?;

        let record = ExecutionRecord {
            id: id.clone(),
            execution_type: exec_type.as_str().to_string(),
            request_origin: origin.to_string(),
            status: ExecutionStatus::Running.as_str().to_string(),
            current_stage: None,
            progress_pct: 0.0,
            processed_count: 0,
            failed_count: 0,
            diagnostic_message: None,
            metadata_json: meta_str,
            started_at: now_str,
            completed_at: None,
            duration_ms: None,
        };

        {
            let mut guard = self.cancellations.write().await;
            guard.insert(id.clone(), cancel_flag.clone());
        }

        let _ = self.event_tx.send(ExecutionEventMessage::ExecutionCreated(record.clone()));

        info!("[ExecEngine] Execução #{} iniciada: {} (origem: {})", id, exec_type, origin);

        Ok(ExecutionHandle {
            id,
            execution_type: exec_type,
            engine: self.clone(),
            cancel_flag,
            start_instant: Instant::now(),
        })
    }

    /// Atualiza progresso e contadores da execução
    pub async fn update_progress(
        &self,
        execution_id: &str,
        progress_pct: f64,
        current_stage: Option<&str>,
        processed_count: i64,
        failed_count: i64,
    ) -> Result<()> {
        sqlx::query(
            "UPDATE executions
             SET progress_pct = ?1, current_stage = ?2, processed_count = ?3, failed_count = ?4
             WHERE id = ?5",
        )
        .bind(progress_pct.clamp(0.0, 100.0))
        .bind(current_stage)
        .bind(processed_count)
        .bind(failed_count)
        .bind(execution_id)
        .execute(self.db.pool())
        .await
        .with_context(|| format!("Erro ao atualizar progresso da execução {}", execution_id))?;

        if let Ok(Some(updated)) = self.get_execution(execution_id).await {
            let _ = self.event_tx.send(ExecutionEventMessage::ExecutionUpdated(updated));
        }

        Ok(())
    }

    /// Inicia uma etapa na tabela `execution_stages`
    pub async fn start_stage(
        &self,
        execution_id: &str,
        stage_name: &str,
    ) -> Result<StageRecord> {
        let now_str = Utc::now().to_rfc3339();
        let res = sqlx::query(
            "INSERT INTO execution_stages (
                execution_id, stage_name, status, started_at, items_in, items_out
            ) VALUES (?1, ?2, ?3, ?4, 0, 0)",
        )
        .bind(execution_id)
        .bind(stage_name)
        .bind(StageStatus::Running.as_str())
        .bind(&now_str)
        .execute(self.db.pool())
        .await
        .with_context(|| format!("Erro ao criar etapa '{}' para {}", stage_name, execution_id))?;

        let stage_id = res.last_insert_rowid();

        let stage = StageRecord {
            id: stage_id,
            execution_id: execution_id.to_string(),
            stage_name: stage_name.to_string(),
            status: StageStatus::Running.as_str().to_string(),
            started_at: now_str,
            completed_at: None,
            duration_ms: None,
            items_in: 0,
            items_out: 0,
            error_message: None,
        };

        let _ = self.event_tx.send(ExecutionEventMessage::StageStarted(stage.clone()));

        Ok(stage)
    }

    /// Finaliza uma etapa (sucesso ou falha)
    pub async fn finish_stage(
        &self,
        stage_id: i64,
        status: StageStatus,
        duration_ms: i64,
        items_in: i64,
        items_out: i64,
        error_message: Option<&str>,
    ) -> Result<Option<StageRecord>> {
        let now_str = Utc::now().to_rfc3339();
        sqlx::query(
            "UPDATE execution_stages
             SET status = ?1, completed_at = ?2, duration_ms = ?3, items_in = ?4, items_out = ?5, error_message = ?6
             WHERE id = ?7",
        )
        .bind(status.as_str())
        .bind(&now_str)
        .bind(duration_ms)
        .bind(items_in)
        .bind(items_out)
        .bind(error_message)
        .bind(stage_id)
        .execute(self.db.pool())
        .await
        .with_context(|| format!("Erro ao finalizar etapa #{}", stage_id))?;

        let maybe_stage = sqlx::query_as::<_, StageRecord>(
            "SELECT id, execution_id, stage_name, status, started_at, completed_at, duration_ms, items_in, items_out, error_message
             FROM execution_stages WHERE id = ?1",
        )
        .bind(stage_id)
        .fetch_optional(self.db.pool())
        .await?;

        if let Some(ref stage) = maybe_stage {
            let _ = self.event_tx.send(ExecutionEventMessage::StageCompleted(stage.clone()));
        }

        Ok(maybe_stage)
    }

    /// Registra um evento estruturado no banco e canal
    pub async fn log_event(
        &self,
        execution_id: &str,
        stage: &str,
        source: Option<&str>,
        level: EventLevel,
        status: Option<&str>,
        message: &str,
        details: Option<&str>,
        duration_ms: Option<i64>,
    ) -> Result<EventRecord> {
        let event_id = generate_event_id();
        let now_str = Utc::now().to_rfc3339();

        match level {
            EventLevel::Debug => debug!("[{}] {} (stage={})", execution_id, message, stage),
            EventLevel::Info => info!("[{}] {} (stage={})", execution_id, message, stage),
            EventLevel::Warning => warn!("[{}] [WARN] {} (stage={})", execution_id, message, stage),
            EventLevel::Error => error!("[{}] [ERROR] {} (stage={})", execution_id, message, stage),
        }

        let res = sqlx::query(
            "INSERT INTO execution_events (
                execution_id, event_id, stage, source, level, status, message, details, created_at, duration_ms
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
        )
        .bind(execution_id)
        .bind(&event_id)
        .bind(stage)
        .bind(source)
        .bind(level.as_str())
        .bind(status)
        .bind(message)
        .bind(details)
        .bind(&now_str)
        .bind(duration_ms)
        .execute(self.db.pool())
        .await
        .with_context(|| format!("Erro ao persistir evento {}", event_id))?;

        let record = EventRecord {
            id: res.last_insert_rowid(),
            execution_id: execution_id.to_string(),
            event_id,
            stage: stage.to_string(),
            source: source.map(|s| s.to_string()),
            level: level.as_str().to_string(),
            status: status.map(|s| s.to_string()),
            message: message.to_string(),
            details: details.map(|d| d.to_string()),
            created_at: now_str,
            duration_ms,
        };

        let _ = self.event_tx.send(ExecutionEventMessage::EventLogged(record.clone()));

        Ok(record)
    }

    /// Registra uma métrica de volume ou desempenho
    pub async fn record_metric(
        &self,
        execution_id: &str,
        metric_name: &str,
        metric_value: f64,
        metric_unit: &str,
    ) -> Result<MetricRecord> {
        let now_str = Utc::now().to_rfc3339();

        let res = sqlx::query(
            "INSERT INTO execution_metrics (
                execution_id, metric_name, metric_value, metric_unit, created_at
            ) VALUES (?1, ?2, ?3, ?4, ?5)",
        )
        .bind(execution_id)
        .bind(metric_name)
        .bind(metric_value)
        .bind(metric_unit)
        .bind(&now_str)
        .execute(self.db.pool())
        .await
        .with_context(|| format!("Erro ao registrar métrica '{}'", metric_name))?;

        let record = MetricRecord {
            id: res.last_insert_rowid(),
            execution_id: execution_id.to_string(),
            metric_name: metric_name.to_string(),
            metric_value,
            metric_unit: metric_unit.to_string(),
            created_at: now_str,
        };

        let _ = self.event_tx.send(ExecutionEventMessage::MetricRecorded(record.clone()));

        Ok(record)
    }

    /// Finaliza uma execução com status terminal (COMPLETED, PARTIALLY_COMPLETED, FAILED, CANCELLED)
    pub async fn finish_execution(
        &self,
        execution_id: &str,
        final_status: ExecutionStatus,
        duration_ms: i64,
        processed_count: Option<i64>,
        failed_count: Option<i64>,
        diagnostic_message: Option<&str>,
    ) -> Result<Option<ExecutionRecord>> {
        let now_str = Utc::now().to_rfc3339();

        sqlx::query(
            "UPDATE executions
             SET status = ?1, completed_at = ?2, duration_ms = ?3,
                 processed_count = COALESCE(?4, processed_count),
                 failed_count = COALESCE(?5, failed_count),
                 diagnostic_message = COALESCE(?6, diagnostic_message),
                 progress_pct = CASE WHEN ?1 = 'COMPLETED' THEN 100.0 ELSE progress_pct END
             WHERE id = ?7",
        )
        .bind(final_status.as_str())
        .bind(&now_str)
        .bind(duration_ms)
        .bind(processed_count)
        .bind(failed_count)
        .bind(diagnostic_message)
        .bind(execution_id)
        .execute(self.db.pool())
        .await
        .with_context(|| format!("Erro ao finalizar execução {}", execution_id))?;

        {
            let mut guard = self.cancellations.write().await;
            guard.remove(execution_id);
        }

        let updated = self.get_execution(execution_id).await?;
        if let Some(ref record) = updated {
            let _ = self.event_tx.send(ExecutionEventMessage::ExecutionUpdated(record.clone()));
            info!(
                "[ExecEngine] Execução #{} finalizada com status {} em {}ms (processados={}, falhas={})",
                execution_id, final_status, duration_ms, record.processed_count, record.failed_count
            );
        }

        Ok(updated)
    }

    /// Cancela uma execução em andamento
    pub async fn cancel_execution(&self, execution_id: &str) -> Result<bool> {
        let cancel_flag = {
            let guard = self.cancellations.read().await;
            guard.get(execution_id).cloned()
        };

        if let Some(flag) = cancel_flag {
            flag.store(true, Ordering::SeqCst);
        }

        let res = sqlx::query(
            "UPDATE executions
             SET status = 'CANCELLED', completed_at = datetime('now'),
                 diagnostic_message = 'Execução cancelada pelo usuário ou sistema.'
             WHERE id = ?1 AND status IN ('PENDING', 'RUNNING')",
        )
        .bind(execution_id)
        .execute(self.db.pool())
        .await?;

        let affected = res.rows_affected() > 0;
        if affected {
            let _ = self.log_event(
                execution_id,
                "engine",
                None,
                EventLevel::Warning,
                Some("CANCELLED"),
                "Execução cancelada a pedido do usuário.",
                None,
                None,
            ).await;

            if let Ok(Some(updated)) = self.get_execution(execution_id).await {
                let _ = self.event_tx.send(ExecutionEventMessage::ExecutionUpdated(updated));
            }
        }

        Ok(affected)
    }

    pub async fn get_execution(&self, execution_id: &str) -> Result<Option<ExecutionRecord>> {
        let record = sqlx::query_as::<_, ExecutionRecord>(
            "SELECT id, execution_type, request_origin, status, current_stage,
                    progress_pct, processed_count, failed_count, diagnostic_message,
                    metadata_json, started_at, completed_at, duration_ms
             FROM executions WHERE id = ?1",
        )
        .bind(execution_id)
        .fetch_optional(self.db.pool())
        .await?;

        Ok(record)
    }

    pub async fn get_execution_detail(&self, execution_id: &str) -> Result<Option<ExecutionDetail>> {
        let Some(execution) = self.get_execution(execution_id).await? else {
            return Ok(None);
        };

        let stages = sqlx::query_as::<_, StageRecord>(
            "SELECT id, execution_id, stage_name, status, started_at, completed_at, duration_ms, items_in, items_out, error_message
             FROM execution_stages WHERE execution_id = ?1 ORDER BY id ASC",
        )
        .bind(execution_id)
        .fetch_all(self.db.pool())
        .await?;

        let events = sqlx::query_as::<_, EventRecord>(
            "SELECT id, execution_id, event_id, stage, source, level, status, message, details, created_at, duration_ms
             FROM execution_events WHERE execution_id = ?1 ORDER BY id ASC",
        )
        .bind(execution_id)
        .fetch_all(self.db.pool())
        .await?;

        let metrics = sqlx::query_as::<_, MetricRecord>(
            "SELECT id, execution_id, metric_name, metric_value, metric_unit, created_at
             FROM execution_metrics WHERE execution_id = ?1 ORDER BY id ASC",
        )
        .bind(execution_id)
        .fetch_all(self.db.pool())
        .await?;

        Ok(Some(ExecutionDetail {
            execution,
            stages,
            events,
            metrics,
        }))
    }

    pub async fn list_executions(
        &self,
        limit: i64,
        offset: i64,
        type_filter: Option<&str>,
        status_filter: Option<&str>,
    ) -> Result<Vec<ExecutionRecord>> {
        let eff_limit = limit.clamp(1, 200);
        let eff_offset = offset.max(0);

        let records = sqlx::query_as::<_, ExecutionRecord>(
            "SELECT id, execution_type, request_origin, status, current_stage,
                    progress_pct, processed_count, failed_count, diagnostic_message,
                    metadata_json, started_at, completed_at, duration_ms
             FROM executions
             WHERE (?1 IS NULL OR execution_type = ?1)
               AND (?2 IS NULL OR status = ?2)
             ORDER BY started_at DESC, id DESC
             LIMIT ?3 OFFSET ?4",
        )
        .bind(type_filter)
        .bind(status_filter)
        .bind(eff_limit)
        .bind(eff_offset)
        .fetch_all(self.db.pool())
        .await?;

        Ok(records)
    }

    /// Retorna métricas consolidadas para o Dashboard Geral da Central de Execuções (Sprint 14)
    pub async fn get_executions_summary(&self) -> Result<ExecutionsSummary> {
        let row = sqlx::query(
            r#"
            SELECT
                COALESCE(SUM(CASE WHEN status = 'RUNNING' THEN 1 ELSE 0 END), 0) as active_count,
                COALESCE(SUM(CASE WHEN status = 'COMPLETED' THEN 1 ELSE 0 END), 0) as completed_count,
                COALESCE(SUM(CASE WHEN status IN ('FAILED', 'CANCELLED') THEN 1 ELSE 0 END), 0) as failed_count,
                COALESCE(SUM(processed_count), 0) as total_processed,
                COALESCE(SUM(failed_count), 0) as total_failed_items,
                COALESCE(AVG(CASE WHEN duration_ms IS NOT NULL AND duration_ms > 0 THEN duration_ms ELSE NULL END), 0.0) as avg_duration_ms
            FROM executions
            "#
        )
        .fetch_one(self.db.pool())
        .await?;

        let active_executions: i64 = row.try_get("active_count").unwrap_or(0);
        let completed_executions: i64 = row.try_get("completed_count").unwrap_or(0);
        let failed_executions: i64 = row.try_get("failed_count").unwrap_or(0);
        let total_processed: i64 = row.try_get("total_processed").unwrap_or(0);
        let total_failed_items: i64 = row.try_get("total_failed_items").unwrap_or(0);
        let avg_duration_ms: f64 = row.try_get("avg_duration_ms").unwrap_or(0.0);

        let total_collected_jobs: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM jobs")
            .fetch_one(self.db.pool())
            .await
            .unwrap_or(0);

        let total_sources_tracked: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM job_sources")
            .fetch_one(self.db.pool())
            .await
            .unwrap_or(0);

        let ai_row = sqlx::query(
            "SELECT COUNT(*) as ops_count, COALESCE(SUM(total_tokens), 0) as tokens_sum FROM ai_operations"
        )
        .fetch_one(self.db.pool())
        .await;

        let (total_ai_operations, total_ai_tokens) = match ai_row {
            Ok(r) => (
                r.try_get::<i64, _>("ops_count").unwrap_or(0),
                r.try_get::<i64, _>("tokens_sum").unwrap_or(0),
            ),
            Err(_) => (0, 0),
        };

        Ok(ExecutionsSummary {
            active_executions,
            completed_executions,
            failed_executions,
            total_processed,
            total_failed_items,
            avg_duration_ms,
            total_collected_jobs,
            total_sources_tracked,
            total_ai_operations,
            total_ai_tokens,
        })
    }
}

/// Handle ergonômico para manipular uma execução ativa
#[derive(Clone)]
pub struct ExecutionHandle {
    pub id: String,
    pub execution_type: ExecutionType,
    pub engine: Arc<ExecutionEngine>,
    cancel_flag: Arc<AtomicBool>,
    start_instant: Instant,
}

impl ExecutionHandle {
    pub fn id(&self) -> &str {
        &self.id
    }

    pub fn is_cancelled(&self) -> bool {
        self.cancel_flag.load(Ordering::SeqCst)
    }

    pub async fn update_progress(
        &self,
        pct: f64,
        current_stage: Option<&str>,
        processed: i64,
        failed: i64,
    ) -> Result<()> {
        self.engine
            .update_progress(&self.id, pct, current_stage, processed, failed)
            .await
    }

    pub async fn start_stage(&self, stage_name: &str) -> Result<StageHandle> {
        let stage = self.engine.start_stage(&self.id, stage_name).await?;
        let _ = self
            .update_progress(
                0.0,
                Some(stage_name),
                0,
                0,
            )
            .await;

        Ok(StageHandle {
            execution_id: self.id.clone(),
            stage_id: stage.id,
            stage_name: stage_name.to_string(),
            engine: self.engine.clone(),
            start_instant: Instant::now(),
        })
    }

    pub async fn emit(
        &self,
        level: EventLevel,
        stage: &str,
        source: Option<&str>,
        message: &str,
        details: Option<&str>,
    ) -> Result<EventRecord> {
        self.engine
            .log_event(
                &self.id,
                stage,
                source,
                level,
                None,
                message,
                details,
                Some(self.start_instant.elapsed().as_millis() as i64),
            )
            .await
    }

    pub async fn emit_info(&self, stage: &str, source: Option<&str>, message: &str) {
        let _ = self.emit(EventLevel::Info, stage, source, message, None).await;
    }

    pub async fn emit_warn(&self, stage: &str, source: Option<&str>, message: &str) {
        let _ = self.emit(EventLevel::Warning, stage, source, message, None).await;
    }

    pub async fn emit_error(
        &self,
        stage: &str,
        source: Option<&str>,
        message: &str,
        details: Option<&str>,
    ) {
        let _ = self.emit(EventLevel::Error, stage, source, message, details).await;
    }

    pub async fn record_metric(&self, name: &str, value: f64, unit: &str) -> Result<()> {
        let _ = self.engine.record_metric(&self.id, name, value, unit).await?;
        Ok(())
    }

    pub async fn finish_success(
        &self,
        processed: i64,
        failed: i64,
        message: Option<&str>,
    ) -> Result<()> {
        let elapsed = self.start_instant.elapsed().as_millis() as i64;
        self.record_metric("total_duration_ms", elapsed as f64, "ms").await.ok();
        self.record_metric("processed_records", processed as f64, "count").await.ok();
        self.record_metric("failed_records", failed as f64, "count").await.ok();

        self.engine
            .finish_execution(
                &self.id,
                ExecutionStatus::Completed,
                elapsed,
                Some(processed),
                Some(failed),
                message,
            )
            .await?;
        Ok(())
    }

    pub async fn finish_partial(
        &self,
        processed: i64,
        failed: i64,
        message: &str,
    ) -> Result<()> {
        let elapsed = self.start_instant.elapsed().as_millis() as i64;
        self.record_metric("total_duration_ms", elapsed as f64, "ms").await.ok();
        self.record_metric("processed_records", processed as f64, "count").await.ok();
        self.record_metric("failed_records", failed as f64, "count").await.ok();

        self.engine
            .finish_execution(
                &self.id,
                ExecutionStatus::PartiallyCompleted,
                elapsed,
                Some(processed),
                Some(failed),
                Some(message),
            )
            .await?;
        Ok(())
    }

    pub async fn finish_failure(&self, error_message: &str) -> Result<()> {
        let elapsed = self.start_instant.elapsed().as_millis() as i64;
        self.emit_error("general", None, error_message, None).await;

        self.engine
            .finish_execution(
                &self.id,
                ExecutionStatus::Failed,
                elapsed,
                None,
                None,
                Some(error_message),
            )
            .await?;
        Ok(())
    }
}

/// Handle ergonômico para uma etapa individual
pub struct StageHandle {
    pub execution_id: String,
    pub stage_id: i64,
    pub stage_name: String,
    pub engine: Arc<ExecutionEngine>,
    pub start_instant: Instant,
}

impl StageHandle {
    pub async fn complete(&self, items_in: i64, items_out: i64) -> Result<()> {
        let dur = self.start_instant.elapsed().as_millis() as i64;
        self.engine
            .finish_stage(
                self.stage_id,
                StageStatus::Completed,
                dur,
                items_in,
                items_out,
                None,
            )
            .await?;
        Ok(())
    }

    pub async fn fail(&self, err_msg: &str) -> Result<()> {
        let dur = self.start_instant.elapsed().as_millis() as i64;
        self.engine
            .finish_stage(
                self.stage_id,
                StageStatus::Failed,
                dur,
                0,
                0,
                Some(err_msg),
            )
            .await?;
        Ok(())
    }

    pub async fn skip(&self) -> Result<()> {
        let dur = self.start_instant.elapsed().as_millis() as i64;
        self.engine
            .finish_stage(
                self.stage_id,
                StageStatus::Skipped,
                dur,
                0,
                0,
                None,
            )
            .await?;
        Ok(())
    }
}
