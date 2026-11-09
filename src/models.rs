use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use sqlx::FromRow;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RawJobListing {
    pub external_id: String,
    pub source: String,
    pub title: String,
    pub company: String,
    pub location: String,
    pub url: String,
    pub raw_description: String,
    #[serde(default)]
    pub published_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ExtractedJobMetadata {
    pub external_id: String,
    #[serde(default = "default_nao_informado")]
    pub work_model: String,
    #[serde(default = "default_nao_informado")]
    pub contract_type: String,
    #[serde(default = "default_nao_informado")]
    pub seniority: String,
    #[serde(default)]
    pub shift_schedule: Option<String>,
    #[serde(default)]
    pub required_skills: Vec<String>,
    #[serde(default)]
    pub salary_estimate_brl: Option<String>,
    #[serde(default)]
    pub summary: String,
    #[serde(default)]
    pub red_flags: Vec<String>,
}

fn default_nao_informado() -> String {
    "Nao Informado".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct BatchExtractionResponse {
    pub jobs: Vec<ExtractedJobMetadata>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct MatchAnalysisResult {
    pub match_score: f32,
    #[serde(default)]
    pub matching_skills: Vec<String>,
    #[serde(default)]
    pub missing_skills: Vec<String>,
    #[serde(default)]
    pub ats_suggestions: Vec<String>,
    #[serde(default)]
    pub shift_and_location_verdict: String,
    #[serde(default)]
    pub detailed_analysis_markdown: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct JobRecord {
    pub id: i64,
    pub dedup_hash: String,
    pub external_id: Option<String>,
    pub source: Option<String>,
    pub title: Option<String>,
    pub company: Option<String>,
    pub location: Option<String>,
    pub url: Option<String>,
    pub raw_description: Option<String>,
    pub is_enriched: i64,
    pub work_model: Option<String>,
    pub contract_type: Option<String>,
    pub seniority: Option<String>,
    pub shift_schedule: Option<String>,
    pub salary_estimate_brl: Option<String>,
    pub required_skills_json: Option<String>,
    pub red_flags_json: Option<String>,
    pub ai_summary: Option<String>,
    pub embedding_json: Option<String>,
    pub application_status: Option<String>,
    pub match_score: Option<f64>,
    pub match_analysis_md: Option<String>,
    pub published_at: Option<String>,
    pub created_at: Option<String>,
}

impl JobRecord {
    pub fn required_skills(&self) -> Vec<String> {
        self.required_skills_json
            .as_deref()
            .and_then(|s| serde_json::from_str::<Vec<String>>(s).ok())
            .unwrap_or_default()
    }

    pub fn red_flags(&self) -> Vec<String> {
        self.red_flags_json
            .as_deref()
            .and_then(|s| serde_json::from_str::<Vec<String>>(s).ok())
            .unwrap_or_default()
    }

    pub fn embedding(&self) -> Option<Vec<f32>> {
        self.embedding_json
            .as_deref()
            .and_then(|s| serde_json::from_str::<Vec<f32>>(s).ok())
    }

    pub fn title_str(&self) -> &str {
        self.title.as_deref().unwrap_or("Sem Título")
    }

    pub fn company_str(&self) -> &str {
        self.company.as_deref().unwrap_or("Empresa N/D")
    }

    pub fn location_str(&self) -> &str {
        self.location.as_deref().unwrap_or("Local N/D")
    }

    pub fn work_model_str(&self) -> &str {
        self.work_model.as_deref().unwrap_or("Nao Informado")
    }

    pub fn shift_str(&self) -> &str {
        self.shift_schedule.as_deref().unwrap_or("N/I")
    }

    pub fn status_str(&self) -> &str {
        self.application_status.as_deref().unwrap_or("Descoberta")
    }

    pub fn to_raw_listing(&self) -> RawJobListing {
        RawJobListing {
            external_id: self
                .external_id
                .clone()
                .unwrap_or_else(|| self.id.to_string()),
            source: self.source.clone().unwrap_or_else(|| "db".to_string()),
            title: self.title_str().to_string(),
            company: self.company_str().to_string(),
            location: self.location_str().to_string(),
            url: self.url.clone().unwrap_or_default(),
            raw_description: self.raw_description.clone().unwrap_or_default(),
            published_at: self.published_at.clone(),
        }
    }

    pub fn build_embedding_text(&self, meta: &ExtractedJobMetadata) -> String {
        format!(
            "Título: {}\nEmpresa: {}\nLocal: {}\nModalidade: {}\nContrato: {}\nSenioridade: {}\nEscala/Turno: {}\nSkills: {}\nResumo: {}",
            self.title_str(),
            self.company_str(),
            self.location_str(),
            meta.work_model,
            meta.contract_type,
            meta.seniority,
            meta.shift_schedule.as_deref().unwrap_or("Não informado"),
            meta.required_skills.join(", "),
            meta.summary
        )
    }
}

#[derive(Debug, Clone, Default)]
pub struct JobFilter {
    pub query: Option<String>,
    pub work_model: Option<String>,
    pub seniority: Option<String>,
    pub shift: Option<String>,
    pub status: Option<String>,
    pub only_enriched: bool,
}

#[derive(Debug, Clone, Default)]
pub struct DbStats {
    pub total_jobs: i64,
    pub enriched_jobs: i64,
    pub saved_or_applied_jobs: i64,
}
