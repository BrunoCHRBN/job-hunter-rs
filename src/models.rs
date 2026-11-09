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
}
