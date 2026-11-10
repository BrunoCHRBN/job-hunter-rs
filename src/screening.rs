use chrono::Utc;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::models::{CandidateProfile, JobRecord};
use crate::normalizer::{
    normalize_company_name, normalize_location, normalize_seniority, normalize_work_model,
    strip_accents,
};

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct ClassifiedRequirement {
    /// "MUST_HAVE" | "NICE_TO_HAVE" | "OPTIONAL" | "UNKNOWN"
    pub tier: String,
    pub description: String,
    /// "explicit" | "inferred" | "unknown"
    pub origin: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
pub struct ScreeningResult {
    /// "PASS" | "REVIEW" | "REJECT"
    pub verdict: String,
    /// Pontuação determinística (0 a 100)
    pub score: f64,
    /// Indica se a vaga deve avançar para a Camada 2 (Análise Semântica com IA)
    pub eligible_for_ai: bool,
    pub criteria_met: Vec<String>,
    pub criteria_failed: Vec<String>,
    pub missing_info: Vec<String>,
    pub classified_requirements: Vec<ClassifiedRequirement>,
    pub justification: String,
    /// Nota harmonizada com o Match Score semântico quando disponível
    pub harmonized_score: Option<f64>,
    pub evaluated_at: String,
}

/// Executa a Camada 1 (Triagem Determinística em Rust, custo zero de IA) e harmoniza
/// com metadados do Dossiê e Match Score (Camada 2) quando presentes.
/// Informações ausentes nunca são tratadas automaticamente como incompatibilidade.
pub fn evaluate_job_screening(
    job: &JobRecord,
    profile: &CandidateProfile,
    preferred_companies_norm: &[String],
    excluded_companies_norm: &[String],
) -> ScreeningResult {
    let mut score: f64 = 75.0;
    let mut hard_reject = false;
    let mut needs_review = false;
    let mut criteria_met = Vec::new();
    let mut criteria_failed = Vec::new();
    let mut missing_info = Vec::new();

    let title = job.title_str();
    let company = job.company_str();
    let location = job.location_str();
    let desc = job.raw_description.as_deref().unwrap_or("");
    let norm_company = normalize_company_name(company);

    // 1. Termos proibidos no título
    if profile.matches_exclusion(title) {
        hard_reject = true;
        score -= 60.0;
        criteria_failed.push(format!(
            "Título '{}' contém termo bloqueado nas regras de exclusão do perfil",
            title
        ));
    } else {
        criteria_met.push("Título livre de termos bloqueados pelo perfil".to_string());
    }

    // 2. Empresas preferidas ou excluídas
    if !norm_company.is_empty()
        && excluded_companies_norm
            .iter()
            .any(|c| !c.is_empty() && (norm_company == *c || norm_company.contains(c)))
    {
        hard_reject = true;
        score -= 60.0;
        criteria_failed.push(format!(
            "Empresa '{}' está na lista de empresas excluídas",
            company
        ));
    } else if !norm_company.is_empty()
        && preferred_companies_norm
            .iter()
            .any(|c| !c.is_empty() && (norm_company == *c || norm_company.contains(c)))
    {
        score += 10.0;
        criteria_met.push(format!(
            "Empresa '{}' está marcada como prioritária/preferida",
            company
        ));
    }

    // 3. Modalidade e Localização
    let raw_wm = job.work_model.as_deref().unwrap_or("Nao Informado");
    let effective_wm = normalize_work_model(raw_wm, location, title, desc);
    let norm_loc = normalize_location(location);

    let accepted_cities: Vec<String> = std::iter::once(profile.base_city.as_str())
        .chain(profile.accepted_region.split(','))
        .map(|c| strip_accents(c.trim()))
        .filter(|c| !c.is_empty())
        .collect();

    let loc_lower = strip_accents(location);
    let is_in_region = accepted_cities
        .iter()
        .any(|city| loc_lower.contains(city) || norm_loc.starts_with(city));

    if effective_wm == "Remoto" || norm_loc == "remoto" {
        if profile.accept_remote == 1 {
            score += 10.0;
            criteria_met.push("Modalidade Remota compatível com o perfil".to_string());
        } else {
            needs_review = true;
            score -= 25.0;
            criteria_failed.push("Vaga Remota, mas perfil está configurado apenas para presencial".to_string());
        }
    } else if is_in_region {
        score += 10.0;
        criteria_met.push(format!(
            "Localização '{}' dentro da região aceita ({})",
            location, profile.base_city
        ));
    } else if norm_loc == "brasil" && effective_wm == "Nao Informado" {
        // Informação de cidade/modalidade ausente -> NÃO penaliza como incompatibilidade
        missing_info.push("Cidade específica e modalidade de trabalho não especificadas".to_string());
    } else {
        // Localidade fora da região aceita
        if effective_wm == "Presencial" {
            hard_reject = true;
            score -= 45.0;
            criteria_failed.push(format!(
                "Vaga Presencial em '{}' fora da região aceita ({})",
                location, profile.accepted_region
            ));
        } else {
            needs_review = true;
            score -= 15.0;
            criteria_failed.push(format!(
                "Localidade '{}' fora da região base (modalidade: {})",
                location, effective_wm
            ));
        }
    }

    // 4. Escala / Turno
    let shift = job.shift_schedule.as_deref().unwrap_or("");
    let shift_norm = strip_accents(shift.trim());
    if shift_norm.is_empty()
        || shift_norm == "n/i"
        || shift_norm == "nao informado"
        || shift_norm == "nao especificado"
    {
        missing_info.push("Escala ou turno de trabalho não informado no anúncio".to_string());
    } else {
        criteria_met.push(format!("Escala/Turno identificado: {}", shift));
    }

    // 5. Senioridade
    let sen_raw = job.seniority.as_deref().unwrap_or("Nao Informado");
    let effective_sen = normalize_seniority(sen_raw, title);
    if effective_sen == "Nao Informado" {
        missing_info.push("Nível de senioridade não especificado explicitamente".to_string());
    } else {
        criteria_met.push(format!("Senioridade classificada: {}", effective_sen));
    }

    // 6. Faixa salarial (quando informada vs ausente)
    let sal_raw = job.salary_estimate_brl.as_deref().unwrap_or("");
    let sal_norm = strip_accents(sal_raw.trim());
    if sal_norm.is_empty()
        || sal_norm == "nao informado"
        || sal_norm == "a combinar"
        || sal_norm.contains("nao divulgad")
    {
        missing_info.push("Remuneração não divulgada (ausência tratada sem penalidade)".to_string());
    } else {
        if let (Some(job_val), Some(min_val)) = (
            extract_first_currency_value(sal_raw),
            extract_first_currency_value(&profile.min_salary_brl),
        ) {
            if min_val > 0.0 && job_val < min_val * 0.85 {
                needs_review = true;
                score -= 15.0;
                criteria_failed.push(format!(
                    "Salário estimado (R$ {:.0}) abaixo do mínimo desejado (R$ {:.0})",
                    job_val, min_val
                ));
            } else {
                score += 5.0;
                criteria_met.push(format!("Faixa salarial informada compatível ({})", sal_raw));
            }
        } else {
            criteria_met.push(format!("Remuneração informada: {}", sal_raw));
        }
    }

    // 7. Classificação estruturada de requisitos (MUST_HAVE, NICE_TO_HAVE, OPTIONAL, UNKNOWN)
    let classified_requirements = classify_job_requirements(job);

    // 8. Alertas críticos do Dossiê (quando enriquecido)
    if let Some(dossier) = job.dossier() {
        for alert in &dossier.alerts {
            if alert.level() == "critical" {
                needs_review = true;
                score -= 10.0;
                criteria_failed.push(format!("Alerta crítico na vaga: {}", alert.text));
            }
        }
    }

    let clamped_score = score.clamp(0.0, 100.0);

    let verdict = if hard_reject || clamped_score < 35.0 {
        "REJECT"
    } else if needs_review || !criteria_failed.is_empty() || clamped_score < 65.0 {
        "REVIEW"
    } else {
        "PASS"
    }
    .to_string();

    let eligible_for_ai = verdict != "REJECT";

    // 9. Harmonização com o Match Score semântico (quando disponível)
    let harmonized_score = job.match_score.map(|ai_score| {
        if verdict == "REJECT" {
            (ai_score * 0.4 + clamped_score * 0.6).min(42.0)
        } else {
            (ai_score * 0.7 + clamped_score * 0.3).clamp(0.0, 100.0)
        }
    });

    let justification = build_screening_justification(
        &verdict,
        clamped_score,
        harmonized_score,
        &criteria_met,
        &criteria_failed,
        &missing_info,
    );

    ScreeningResult {
        verdict,
        score: clamped_score,
        eligible_for_ai,
        criteria_met,
        criteria_failed,
        missing_info,
        classified_requirements,
        justification,
        harmonized_score,
        evaluated_at: Utc::now().to_rfc3339(),
    }
}

/// Classifica os requisitos da vaga em MUST_HAVE, NICE_TO_HAVE, OPTIONAL e UNKNOWN.
pub fn classify_job_requirements(job: &JobRecord) -> Vec<ClassifiedRequirement> {
    let mut out = Vec::new();

    if let Some(dossier) = job.dossier() {
        for req in &dossier.sections.required {
            if !req.trim().is_empty() {
                out.push(ClassifiedRequirement {
                    tier: "MUST_HAVE".to_string(),
                    description: req.trim().to_string(),
                    origin: "explicit".to_string(),
                });
            }
        }
        for nice in &dossier.sections.nice_to_have {
            if !nice.trim().is_empty() {
                let low = strip_accents(nice);
                let tier = if low.contains("opcional") || low.contains("desejavel") {
                    "OPTIONAL"
                } else {
                    "NICE_TO_HAVE"
                };
                out.push(ClassifiedRequirement {
                    tier: tier.to_string(),
                    description: nice.trim().to_string(),
                    origin: "explicit".to_string(),
                });
            }
        }
    }

    if out.is_empty() {
        for skill in job.required_skills() {
            if !skill.trim().is_empty() {
                out.push(ClassifiedRequirement {
                    tier: "MUST_HAVE".to_string(),
                    description: skill,
                    origin: "inferred".to_string(),
                });
            }
        }
    }

    // Heurística determinística no texto bruto caso a vaga ainda não tenha sido enriquecida pela IA
    if out.is_empty() {
        if let Some(desc) = &job.raw_description {
            for clause in desc.split(&['\n', '.', ';'][..]) {
                let trimmed = clause.trim().trim_start_matches(&['-', '*', '•'][..]).trim();
                if trimmed.len() < 6 || trimmed.len() > 160 {
                    continue;
                }
                let low = strip_accents(trimmed);
                if low.contains("obrigatorio")
                    || low.contains("imprescindivel")
                    || low.contains("requisito:")
                    || low.contains("necessario experiencia")
                {
                    out.push(ClassifiedRequirement {
                        tier: "MUST_HAVE".to_string(),
                        description: trimmed.to_string(),
                        origin: "explicit".to_string(),
                    });
                } else if low.contains("diferencial") || low.contains("sera um plus") {
                    out.push(ClassifiedRequirement {
                        tier: "NICE_TO_HAVE".to_string(),
                        description: trimmed.to_string(),
                        origin: "explicit".to_string(),
                    });
                } else if low.contains("desejavel") || low.contains("opcional") {
                    out.push(ClassifiedRequirement {
                        tier: "OPTIONAL".to_string(),
                        description: trimmed.to_string(),
                        origin: "explicit".to_string(),
                    });
                }
                if out.len() >= 10 {
                    break;
                }
            }
        }
    }

    out
}

fn extract_first_currency_value(text: &str) -> Option<f64> {
    let mut digits = String::new();
    let mut started = false;
    for ch in text.chars() {
        if ch.is_ascii_digit() {
            digits.push(ch);
            started = true;
        } else if started && (ch == '.' || ch == ',') {
            digits.push(ch);
        } else if started {
            break;
        }
    }
    if digits.is_empty() {
        return None;
    }
    // Converte formato brasileiro "3.500,00" -> "3500.00"
    let normalized = if digits.contains(',') && digits.contains('.') {
        digits.replace('.', "").replace(',', ".")
    } else if digits.contains(',') {
        digits.replace(',', ".")
    } else if digits.matches('.').count() == 1 && digits.ends_with("000") {
        digits.replace('.', "")
    } else {
        digits
    };
    normalized.parse::<f64>().ok()
}

fn build_screening_justification(
    verdict: &str,
    score: f64,
    harmonized: Option<f64>,
    met: &[String],
    failed: &[String],
    missing: &[String],
) -> String {
    let label = match verdict {
        "PASS" => "Aprovada na triagem determinística (PASS)",
        "REVIEW" => "Requer revisão manual (REVIEW)",
        _ => "Rejeitada nos filtros determinísticos (REJECT)",
    };

    let score_part = match harmonized {
        Some(h) => format!(
            "{} — Score Triagem: {:.0}/100 | Score Harmonizado (IA + Regras): {:.0}/100.",
            label, score, h
        ),
        None => format!("{} — Score Triagem: {:.0}/100.", label, score),
    };

    let failed_part = if failed.is_empty() {
        "Nenhum impedimento determinístico.".to_string()
    } else {
        format!("Impedimentos/Alertas: {}.", failed.join("; "))
    };

    let missing_part = if missing.is_empty() {
        String::new()
    } else {
        format!(" Dados omissos (não penalizados): {}.", missing.join(", "))
    };

    format!(
        "{} {} Critérios atendidos: {}.{}",
        score_part,
        failed_part,
        met.len(),
        missing_part
    )
}
