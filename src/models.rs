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

#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
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
    #[serde(default)]
    pub dossier: Dossier,
}

fn default_nao_informado() -> String {
    "Nao Informado".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct BatchExtractionResponse {
    pub jobs: Vec<ExtractedJobMetadata>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
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
    /// "apply" | "maybe" | "skip" (use `verdict_kind()` para o valor final)
    #[serde(default)]
    pub verdict: String,
    #[serde(default)]
    pub verdict_reasons: Vec<String>,
    #[serde(default)]
    pub axes: FitAxes,
    #[serde(default)]
    pub gaps: Vec<GapAction>,
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
    pub dossier_json: Option<String>,
    pub fit_json: Option<String>,
    #[sqlx(default)]
    pub normalized_company: Option<String>,
    #[sqlx(default)]
    pub normalized_title: Option<String>,
    #[sqlx(default)]
    pub canonical_url: Option<String>,
    #[sqlx(default)]
    pub company_id: Option<i64>,
    #[sqlx(default)]
    pub screening_verdict: Option<String>,
    #[sqlx(default)]
    pub screening_score: Option<f64>,
    #[sqlx(default)]
    pub screening_json: Option<String>,
    #[sqlx(default)]
    pub source_status: Option<String>,
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
    pub source: Option<String>,
    pub screening_verdict: Option<String>,
    pub company_id: Option<i64>,
    pub only_enriched: bool,
}

#[derive(Debug, Clone, Default)]
pub struct DbStats {
    pub total_jobs: i64,
    pub enriched_jobs: i64,
    pub saved_or_applied_jobs: i64,
}

// ======================================================================
// Dossiê v2: origem dos dados, alertas com gravidade e encaixe por eixo.
// Tudo é String + helpers de normalização de propósito: se a IA devolver
// um valor fora do vocabulário, o lote inteiro NÃO falha no parse.
// ======================================================================

/// De onde veio um dado extraído da vaga.
#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
pub struct FieldSource {
    /// "explicit" | "inferred" | "absent"
    #[serde(default)]
    pub origin: String,
    /// Trecho literal da descrição (até 12 palavras). Só vale com origin = "explicit".
    #[serde(default)]
    pub evidence: Option<String>,
}

impl FieldSource {
    /// Origem normalizada. "explicit" sem trecho é rebaixado para "inferred".
    pub fn kind(&self) -> &'static str {
        let has_quote = self.evidence.as_deref().map_or(false, |e| !e.trim().is_empty());
        match self.origin.trim().to_lowercase().as_str() {
            "explicit" if has_quote => "explicit",
            "explicit" | "inferred" => "inferred",
            _ => "absent",
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
pub struct Provenance {
    #[serde(default)]
    pub work_model: FieldSource,
    #[serde(default)]
    pub contract_type: FieldSource,
    #[serde(default)]
    pub seniority: FieldSource,
    #[serde(default)]
    pub shift_schedule: FieldSource,
    #[serde(default)]
    pub salary: FieldSource,
}

/// Descrição reorganizada, fiel ao texto original.
#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
pub struct Sections {
    #[serde(default)]
    pub responsibilities: Vec<String>,
    #[serde(default)]
    pub required: Vec<String>,
    #[serde(default)]
    pub nice_to_have: Vec<String>,
    #[serde(default)]
    pub benefits: Vec<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
pub struct Alert {
    /// "critical" | "warning" | "info"
    #[serde(default)]
    pub severity: String,
    #[serde(default)]
    pub text: String,
    /// Trecho literal que sustenta o alerta, quando houver.
    #[serde(default)]
    pub evidence: Option<String>,
}

impl Alert {
    pub fn level(&self) -> &'static str {
        match self.severity.trim().to_lowercase().as_str() {
            "critical" => "critical",
            "info" => "info",
            _ => "warning",
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
pub struct Dossier {
    #[serde(default)]
    pub provenance: Provenance,
    #[serde(default)]
    pub sections: Sections,
    #[serde(default)]
    pub alerts: Vec<Alert>,
}

fn squash(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ").to_lowercase()
}

impl Dossier {
    /// Descarta qualquer trecho que NÃO exista de fato na descrição original.
    /// Dados "explicit" sem trecho verificável passam a valer como "inferred".
    pub fn verify_evidence(&mut self, raw_description: &str) {
        let hay = squash(raw_description);
        let check = |src: &mut FieldSource| {
            if let Some(q) = &src.evidence {
                if q.trim().is_empty() || !hay.contains(&squash(q)) {
                    src.evidence = None;
                }
            }
            src.origin = src.kind().to_string();
        };
        let p = &mut self.provenance;
        check(&mut p.work_model);
        check(&mut p.contract_type);
        check(&mut p.seniority);
        check(&mut p.shift_schedule);
        check(&mut p.salary);
        for a in &mut self.alerts {
            if let Some(q) = &a.evidence {
                if q.trim().is_empty() || !hay.contains(&squash(q)) {
                    a.evidence = None;
                }
            }
            a.severity = a.level().to_string();
        }
    }

    /// O que o texto da vaga não informa (para o bloco "O que o texto não diz").
    pub fn missing(&self) -> Vec<&'static str> {
        let p = &self.provenance;
        let mut m = Vec::new();
        if p.salary.kind() == "absent" {
            m.push("salario");
        }
        if p.shift_schedule.kind() == "absent" {
            m.push("escala");
        }
        if p.work_model.kind() == "absent" {
            m.push("modalidade");
        }
        if p.contract_type.kind() == "absent" {
            m.push("contrato");
        }
        if self.sections.benefits.is_empty() {
            m.push("beneficios");
        }
        m
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
pub struct FitAxis {
    /// "strong" | "partial" | "weak" | "unknown"
    #[serde(default)]
    pub level: String,
    #[serde(default)]
    pub note: String,
}

impl FitAxis {
    pub fn kind(&self) -> &'static str {
        match self.level.trim().to_lowercase().as_str() {
            "strong" => "strong",
            "partial" => "partial",
            "weak" => "weak",
            _ => "unknown",
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
pub struct FitAxes {
    #[serde(default)]
    pub skills: FitAxis,
    #[serde(default)]
    pub logistics: FitAxis,
    #[serde(default)]
    pub seniority: FitAxis,
    #[serde(default)]
    pub salary: FitAxis,
    #[serde(default)]
    pub experience: FitAxis,
    #[serde(default)]
    pub responsibilities: FitAxis,
}

/// Uma lacuna do currículo e o que fazer com ela.
#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
pub struct GapAction {
    #[serde(default)]
    pub skill: String,
    /// "has_unlisted" (há indício no currículo) | "study" | "ignore"
    #[serde(default)]
    pub suggested: String,
    #[serde(default)]
    pub why: String,
}

impl MatchAnalysisResult {
    /// "apply" | "maybe" | "skip". Vale o mais conservador entre a IA e a nota,
    /// e nunca "apply" com logística fraca.
    pub fn verdict_kind(&self) -> &'static str {
        let by_score = if self.match_score >= 70.0 {
            "apply"
        } else if self.match_score >= 45.0 {
            "maybe"
        } else {
            "skip"
        };
        let ai = match self.verdict.trim().to_lowercase().as_str() {
            "apply" => "apply",
            "maybe" => "maybe",
            "skip" => "skip",
            _ => by_score,
        };
        let rank = |v: &str| -> u8 {
            match v {
                "apply" => 2,
                "maybe" => 1,
                _ => 0,
            }
        };
        let mut v = if rank(ai) < rank(by_score) { ai } else { by_score };
        if v == "apply" && self.axes.logistics.kind() == "weak" {
            v = "maybe";
        }
        v
    }
}

impl JobRecord {
    /// None quando a vaga foi enriquecida antes do dossiê v2.
    pub fn dossier(&self) -> Option<Dossier> {
        self.dossier_json
            .as_deref()
            .and_then(|s| serde_json::from_str::<Dossier>(s).ok())
    }

    pub fn fit(&self) -> Option<MatchAnalysisResult> {
        self.fit_json
            .as_deref()
            .and_then(|s| serde_json::from_str::<MatchAnalysisResult>(s).ok())
    }

    pub fn screening(&self) -> Option<crate::screening::ScreeningResult> {
        self.screening_json
            .as_deref()
            .and_then(|s| serde_json::from_str::<crate::screening::ScreeningResult>(s).ok())
    }
}

// ======================================================================
// Automação Contínua (Zero-Touch) e Perfil Editável de Critérios
// ======================================================================

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct CandidateProfile {
    #[serde(default = "default_profile_id")]
    pub id: i64,
    #[serde(default = "default_base_city")]
    pub base_city: String,
    #[serde(default = "default_accepted_region")]
    pub accepted_region: String,
    #[serde(default = "default_one")]
    pub accept_remote: i64,
    #[serde(default = "default_accepted_shifts")]
    pub accepted_shifts: String,
    #[serde(default)]
    pub min_salary_brl: String,
    #[serde(default = "default_exclude_terms")]
    pub exclude_terms: String,
    #[serde(default = "default_one")]
    pub auto_enrich: i64,
    #[serde(default = "default_auto_match_threshold")]
    pub auto_match_threshold: f64,
    #[serde(default = "default_one")]
    pub notify_on_apply: i64,
}

fn default_profile_id() -> i64 {
    1
}
fn default_base_city() -> String {
    "Araraquara".to_string()
}
fn default_accepted_region() -> String {
    "Araraquara, São Carlos, Matão, Américo Brasiliense, Ibaté".to_string()
}
fn default_one() -> i64 {
    1
}
fn default_accepted_shifts() -> String {
    "Comercial (5x2), Escala 6x2, 12x36, Turnos Rotativos".to_string()
}
fn default_exclude_terms() -> String {
    "vendedor de loja, açougueiro, confeiteiro, manicure, corretor de imóveis, pedagogo, farmacêutico".to_string()
}
fn default_auto_match_threshold() -> f64 {
    0.62
}

impl Default for CandidateProfile {
    fn default() -> Self {
        Self {
            id: 1,
            base_city: default_base_city(),
            accepted_region: default_accepted_region(),
            accept_remote: 1,
            accepted_shifts: default_accepted_shifts(),
            min_salary_brl: String::new(),
            exclude_terms: default_exclude_terms(),
            auto_enrich: 1,
            auto_match_threshold: default_auto_match_threshold(),
            notify_on_apply: 1,
        }
    }
}

impl CandidateProfile {
    /// Retorna `true` se o título da vaga contiver algum termo excluído pelo usuário.
    pub fn matches_exclusion(&self, title: &str) -> bool {
        let title_norm = strip_accents_lower(title);
        self.exclude_terms
            .split(',')
            .map(|t| strip_accents_lower(t.trim()))
            .filter(|t| !t.is_empty())
            .any(|term| title_norm.contains(&term))
    }
}

fn strip_accents_lower(s: &str) -> String {
    s.to_lowercase()
        .chars()
        .map(|c| match c {
            'á' | 'à' | 'â' | 'ã' => 'a',
            'é' | 'ê' => 'e',
            'í' => 'i',
            'ó' | 'ô' | 'õ' => 'o',
            'ú' | 'ü' => 'u',
            'ç' => 'c',
            other => other,
        })
        .collect()
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct SavedSearch {
    pub id: i64,
    pub query: String,
    pub location: String,
    pub remote: i64,
    pub enabled: i64,
    pub interval_minutes: i64,
    pub last_run_at: Option<String>,
    pub last_found_count: i64,
}

// ======================================================================
// Sprints 09 a 13: Coletores, Origens Múltiplas, Empresas e Telemetria IA
// ======================================================================

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct CollectorConfig {
    pub source_id: String,
    pub display_name: String,
    pub group_name: String,
    pub enabled: i64,
    pub timeout_secs: i64,
    pub default_limit: i64,
    pub strategy_notes: String,
    pub last_status: Option<String>,
    pub last_run_at: Option<String>,
    pub last_count: i64,
    pub last_error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct JobSourceRecord {
    pub id: i64,
    pub job_id: i64,
    pub source: String,
    pub external_id: String,
    pub original_url: String,
    pub first_seen_at: String,
    pub last_seen_at: String,
    pub source_status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct CompanyRecord {
    pub id: i64,
    pub official_name: String,
    pub normalized_name: String,
    pub website: String,
    pub segment: String,
    pub location: String,
    pub description: String,
    pub is_monitored: i64,
    pub is_preferred: i64,
    pub is_excluded: i64,
    pub created_at: String,
    pub updated_at: String,
    #[sqlx(default)]
    pub active_jobs_count: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct CompanyAliasRecord {
    pub id: i64,
    pub company_id: i64,
    pub alias_name: String,
    pub normalized_alias: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct CompanySourceRecord {
    pub id: i64,
    pub company_id: i64,
    pub source_type: String,
    pub career_url: String,
    pub ats_provider: String,
    pub portal_id: String,
    pub integration_status: String,
    pub last_checked_at: Option<String>,
    pub next_check_at: Option<String>,
    pub last_found_count: i64,
    pub last_error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct AiOperationRecord {
    pub id: i64,
    pub execution_id: Option<String>,
    pub operation_type: String,
    pub model_used: String,
    pub fallback_used: i64,
    pub attempts: i64,
    pub status: String,
    pub records_count: i64,
    pub prompt_tokens: i64,
    pub completion_tokens: i64,
    pub total_tokens: i64,
    pub duration_ms: i64,
    pub error_message: Option<String>,
    pub started_at: String,
    pub completed_at: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AiTelemetrySummary {
    pub total_operations: i64,
    pub total_prompt_tokens: i64,
    pub total_completion_tokens: i64,
    pub total_tokens: i64,
    pub cache_entries: i64,
    pub total_cache_hits: i64,
    pub fallback_count: i64,
    pub error_count: i64,
    pub avg_duration_ms: f64,
    pub recent_operations: Vec<AiOperationRecord>,
}

