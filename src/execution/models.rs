use std::fmt;
use std::str::FromStr;
use serde::{Deserialize, Serialize};
use sqlx::FromRow;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ExecutionType {
    JobSearch,
    CompanySearch,
    CompanyMonitoring,
    AiEnrichment,
    JobScreening,
    MatchAnalysis,
    Automation,
}

impl ExecutionType {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::JobSearch => "JOB_SEARCH",
            Self::CompanySearch => "COMPANY_SEARCH",
            Self::CompanyMonitoring => "COMPANY_MONITORING",
            Self::AiEnrichment => "AI_ENRICHMENT",
            Self::JobScreening => "JOB_SCREENING",
            Self::MatchAnalysis => "MATCH_ANALYSIS",
            Self::Automation => "AUTOMATION",
        }
    }
}

impl fmt::Display for ExecutionType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl FromStr for ExecutionType {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_uppercase().as_str() {
            "JOB_SEARCH" | "JOBSEARCH" => Ok(Self::JobSearch),
            "COMPANY_SEARCH" | "COMPANYSEARCH" => Ok(Self::CompanySearch),
            "COMPANY_MONITORING" | "COMPANYMONITORING" => Ok(Self::CompanyMonitoring),
            "AI_ENRICHMENT" | "AIENRICHMENT" | "ENRICHMENT" => Ok(Self::AiEnrichment),
            "JOB_SCREENING" | "JOBSCREENING" | "SCREENING" => Ok(Self::JobScreening),
            "MATCH_ANALYSIS" | "MATCHANALYSIS" | "MATCH" => Ok(Self::MatchAnalysis),
            "AUTOMATION" => Ok(Self::Automation),
            other => Err(format!("Tipo de execução desconhecido: '{}'", other)),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ExecutionStatus {
    Pending,
    Running,
    Completed,
    PartiallyCompleted,
    Failed,
    Cancelled,
    Retrying,
}

impl ExecutionStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Pending => "PENDING",
            Self::Running => "RUNNING",
            Self::Completed => "COMPLETED",
            Self::PartiallyCompleted => "PARTIALLY_COMPLETED",
            Self::Failed => "FAILED",
            Self::Cancelled => "CANCELLED",
            Self::Retrying => "RETRYING",
        }
    }

    pub fn is_terminal(&self) -> bool {
        matches!(
            self,
            Self::Completed | Self::PartiallyCompleted | Self::Failed | Self::Cancelled
        )
    }
}

impl fmt::Display for ExecutionStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl FromStr for ExecutionStatus {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_uppercase().as_str() {
            "PENDING" => Ok(Self::Pending),
            "RUNNING" => Ok(Self::Running),
            "COMPLETED" => Ok(Self::Completed),
            "PARTIALLY_COMPLETED" => Ok(Self::PartiallyCompleted),
            "FAILED" => Ok(Self::Failed),
            "CANCELLED" => Ok(Self::Cancelled),
            "RETRYING" => Ok(Self::Retrying),
            other => Err(format!("Status de execução desconhecido: '{}'", other)),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum StageStatus {
    Pending,
    Running,
    Completed,
    Failed,
    Skipped,
}

impl StageStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Pending => "PENDING",
            Self::Running => "RUNNING",
            Self::Completed => "COMPLETED",
            Self::Failed => "FAILED",
            Self::Skipped => "SKIPPED",
        }
    }
}

impl fmt::Display for StageStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum EventLevel {
    Debug,
    Info,
    Warning,
    Error,
}

impl EventLevel {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Debug => "DEBUG",
            Self::Info => "INFO",
            Self::Warning => "WARNING",
            Self::Error => "ERROR",
        }
    }
}

impl fmt::Display for EventLevel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct ExecutionRecord {
    pub id: String,
    pub execution_type: String,
    pub request_origin: String,
    pub status: String,
    pub current_stage: Option<String>,
    pub progress_pct: f64,
    pub processed_count: i64,
    pub failed_count: i64,
    pub diagnostic_message: Option<String>,
    pub metadata_json: Option<String>,
    pub started_at: String,
    pub completed_at: Option<String>,
    pub duration_ms: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct StageRecord {
    pub id: i64,
    pub execution_id: String,
    pub stage_name: String,
    pub status: String,
    pub started_at: String,
    pub completed_at: Option<String>,
    pub duration_ms: Option<i64>,
    pub items_in: i64,
    pub items_out: i64,
    pub error_message: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct EventRecord {
    pub id: i64,
    pub execution_id: String,
    pub event_id: String,
    pub stage: String,
    pub source: Option<String>,
    pub level: String,
    pub status: Option<String>,
    pub message: String,
    pub details: Option<String>,
    pub created_at: String,
    pub duration_ms: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct MetricRecord {
    pub id: i64,
    pub execution_id: String,
    pub metric_name: String,
    pub metric_value: f64,
    pub metric_unit: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionDetail {
    pub execution: ExecutionRecord,
    pub stages: Vec<StageRecord>,
    pub events: Vec<EventRecord>,
    pub metrics: Vec<MetricRecord>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ExecutionsSummary {
    pub active_executions: i64,
    pub completed_executions: i64,
    pub failed_executions: i64,
    pub total_processed: i64,
    pub total_failed_items: i64,
    pub avg_duration_ms: f64,
    pub total_collected_jobs: i64,
    pub total_sources_tracked: i64,
    pub total_ai_operations: i64,
    pub total_ai_tokens: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "payload")]
pub enum ExecutionEventMessage {
    ExecutionCreated(ExecutionRecord),
    ExecutionUpdated(ExecutionRecord),
    StageStarted(StageRecord),
    StageCompleted(StageRecord),
    EventLogged(EventRecord),
    MetricRecorded(MetricRecord),
}
