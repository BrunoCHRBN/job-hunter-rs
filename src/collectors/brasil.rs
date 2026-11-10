use std::time::{Duration, Instant};

use reqwest::Client;
use serde::{Deserialize, Serialize};

use crate::models::RawJobListing;
use crate::normalizer::strip_accents;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SourceDiagnosticStatus {
    #[serde(rename = "SUCCESS")]
    Success,
    #[serde(rename = "EMPTY")]
    Empty,
    #[serde(rename = "BLOCKED")]
    Blocked,
    #[serde(rename = "ERROR")]
    Error,
    #[serde(rename = "TIMEOUT")]
    Timeout,
    #[serde(rename = "DISABLED")]
    Disabled,
}

impl SourceDiagnosticStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Success => "SUCCESS",
            Self::Empty => "EMPTY",
            Self::Blocked => "BLOCKED",
            Self::Error => "ERROR",
            Self::Timeout => "TIMEOUT",
            Self::Disabled => "DISABLED",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SourceDiagnosticReport {
    pub source_id: String,
    pub display_name: String,
    pub status: SourceDiagnosticStatus,
    pub count: usize,
    pub duration_ms: u64,
    pub message: String,
}

/// Coleta vagas públicas do portal Vagas.com.br (`https://www.vagas.com.br/vagas-de-{slug}`).
pub async fn fetch_vagas_com_jobs(
    query: &str,
    location: Option<&str>,
    limit: usize,
) -> (Vec<RawJobListing>, SourceDiagnosticReport) {
    let started = Instant::now();
    let slug: String = strip_accents(query.trim())
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { '-' })
        .collect::<String>()
        .split('-')
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("-");

    if slug.is_empty() {
        return (
            Vec::new(),
            SourceDiagnosticReport {
                source_id: "vagas_com".to_string(),
                display_name: "Vagas.com.br".to_string(),
                status: SourceDiagnosticStatus::Empty,
                count: 0,
                duration_ms: 0,
                message: "Termo de busca vazio".to_string(),
            },
        );
    }

    let url = format!("https://www.vagas.com.br/vagas-de-{}", slug);
    let client = match Client::builder()
        .timeout(Duration::from_secs(18))
        .user_agent("Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/124.0.0.0 Safari/537.36")
        .build()
    {
        Ok(c) => c,
        Err(e) => {
            return (
                Vec::new(),
                SourceDiagnosticReport {
                    source_id: "vagas_com".to_string(),
                    display_name: "Vagas.com.br".to_string(),
                    status: SourceDiagnosticStatus::Error,
                    count: 0,
                    duration_ms: started.elapsed().as_millis() as u64,
                    message: format!("Erro ao criar cliente HTTP: {}", e),
                },
            );
        }
    };

    let resp = match client.get(&url).send().await {
        Ok(r) => r,
        Err(err) => {
            let status = if err.is_timeout() {
                SourceDiagnosticStatus::Timeout
            } else {
                SourceDiagnosticStatus::Error
            };
            return (
                Vec::new(),
                SourceDiagnosticReport {
                    source_id: "vagas_com".to_string(),
                    display_name: "Vagas.com.br".to_string(),
                    status,
                    count: 0,
                    duration_ms: started.elapsed().as_millis() as u64,
                    message: format!("Falha ao consultar Vagas.com.br: {}", err),
                },
            );
        }
    };

    let http_status = resp.status();
    if http_status.as_u16() == 403 || http_status.as_u16() == 429 {
        return (
            Vec::new(),
            SourceDiagnosticReport {
                source_id: "vagas_com".to_string(),
                display_name: "Vagas.com.br".to_string(),
                status: SourceDiagnosticStatus::Blocked,
                count: 0,
                duration_ms: started.elapsed().as_millis() as u64,
                message: format!("Acesso limitado pelo servidor (HTTP {})", http_status),
            },
        );
    }

    if !http_status.is_success() {
        return (
            Vec::new(),
            SourceDiagnosticReport {
                source_id: "vagas_com".to_string(),
                display_name: "Vagas.com.br".to_string(),
                status: SourceDiagnosticStatus::Error,
                count: 0,
                duration_ms: started.elapsed().as_millis() as u64,
                message: format!("Resposta inesperada HTTP {}", http_status),
            },
        );
    }

    let html = resp.text().await.unwrap_or_default();
    let parsed_jobs = parse_vagas_com_html(&html, location, limit);
    let dur = started.elapsed().as_millis() as u64;

    let (status, msg) = if parsed_jobs.is_empty() {
        (
            SourceDiagnosticStatus::Empty,
            "Coleta concluída sem resultados compatíveis no Vagas.com.br".to_string(),
        )
    } else {
        (
            SourceDiagnosticStatus::Success,
            format!("{} vagas coletadas no Vagas.com.br", parsed_jobs.len()),
        )
    };

    (
        parsed_jobs.clone(),
        SourceDiagnosticReport {
            source_id: "vagas_com".to_string(),
            display_name: "Vagas.com.br".to_string(),
            status,
            count: parsed_jobs.len(),
            duration_ms: dur,
            message: msg,
        },
    )
}

pub fn parse_vagas_com_html(
    html: &str,
    location_filter: Option<&str>,
    limit: usize,
) -> Vec<RawJobListing> {
    let mut results = Vec::new();
    let loc_tokens: Vec<String> = location_filter
        .unwrap_or("")
        .split(',')
        .map(|s| strip_accents(s.trim()))
        .filter(|s| !s.is_empty() && s != "brasil" && s != "brazil")
        .collect();

    for block in html.split("<li class=\"vaga ").skip(1) {
        if results.len() >= limit {
            break;
        }

        let ext_id = extract_attr(block, "data-id-vaga=\"").unwrap_or_default();
        let href = extract_attr(block, "href=\"/vagas/v")
            .map(|rest| format!("https://www.vagas.com.br/vagas/v{}", rest))
            .unwrap_or_default();
        let title = extract_attr(block, "title=\"")
            .map(|s| decode_html_entities(&s))
            .unwrap_or_default();

        if title.is_empty() || ext_id.is_empty() {
            continue;
        }

        let company = extract_between(block, "<span class=\"emprVaga\">", "</span>")
            .map(|s| clean_html_snippet(&s))
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| "Confidencial".to_string());

        let location = extract_between(block, "<div class=\"vaga-local\">", "</div>")
            .map(|s| clean_html_snippet(&s))
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| "Brasil".to_string());

        if !loc_tokens.is_empty() {
            let loc_norm = strip_accents(&location);
            let is_remote = loc_norm.contains("remoto") || loc_norm.contains("todo o brasil");
            let matches_city = loc_tokens.iter().any(|t| loc_norm.contains(t));
            if !is_remote && !matches_city {
                continue;
            }
        }

        let desc = extract_between(block, "<div class=\"detalhes\">", "</div>")
            .map(|s| clean_html_snippet(&s))
            .unwrap_or_else(|| title.clone());

        results.push(RawJobListing {
            external_id: format!("vagas_{}", ext_id),
            source: "vagas_com".to_string(),
            title,
            company,
            location,
            url: href,
            raw_description: desc,
            published_at: None,
        });
    }

    results
}

/// Coleta vagas na Sólides Vagas ou reporta diagnóstico claro de bloqueio/vazio.
pub async fn fetch_solides_jobs(
    query: &str,
    _location: Option<&str>,
    _limit: usize,
) -> (Vec<RawJobListing>, SourceDiagnosticReport) {
    let started = Instant::now();
    let url = format!(
        "https://vagas.solides.com.br/vagas?search={}",
        urlencoding_simple(query)
    );

    let client = match Client::builder()
        .timeout(Duration::from_secs(15))
        .user_agent("Mozilla/5.0 (X11; Linux x86_64) Chrome/124.0.0.0 Safari/537.36")
        .build()
    {
        Ok(c) => c,
        Err(e) => {
            return (
                Vec::new(),
                SourceDiagnosticReport {
                    source_id: "solides".to_string(),
                    display_name: "Sólides Vagas".to_string(),
                    status: SourceDiagnosticStatus::Error,
                    count: 0,
                    duration_ms: 0,
                    message: e.to_string(),
                },
            );
        }
    };

    match client.get(&url).send().await {
        Ok(resp) => {
            let status = resp.status();
            let dur = started.elapsed().as_millis() as u64;
            if status.as_u16() == 403 || status.as_u16() == 429 {
                return (
                    Vec::new(),
                    SourceDiagnosticReport {
                        source_id: "solides".to_string(),
                        display_name: "Sólides Vagas".to_string(),
                        status: SourceDiagnosticStatus::Blocked,
                        count: 0,
                        duration_ms: dur,
                        message: format!("Acesso limitado por WAF na Sólides (HTTP {})", status),
                    },
                );
            }
            let body = resp.text().await.unwrap_or_default();
            if body.contains("cf-browser-verification") || body.contains("Just a moment...") {
                return (
                    Vec::new(),
                    SourceDiagnosticReport {
                        source_id: "solides".to_string(),
                        display_name: "Sólides Vagas".to_string(),
                        status: SourceDiagnosticStatus::Blocked,
                        count: 0,
                        duration_ms: dur,
                        message: "Proteção anti-bot detectada no portal Sólides".to_string(),
                    },
                );
            }
            (
                Vec::new(),
                SourceDiagnosticReport {
                    source_id: "solides".to_string(),
                    display_name: "Sólides Vagas".to_string(),
                    status: SourceDiagnosticStatus::Empty,
                    count: 0,
                    duration_ms: dur,
                    message: "Portal Sólides requer renderização SPA; nenhuma vaga estática extraída".to_string(),
                },
            )
        }
        Err(e) => (
            Vec::new(),
            SourceDiagnosticReport {
                source_id: "solides".to_string(),
                display_name: "Sólides Vagas".to_string(),
                status: if e.is_timeout() {
                    SourceDiagnosticStatus::Timeout
                } else {
                    SourceDiagnosticStatus::Error
                },
                count: 0,
                duration_ms: started.elapsed().as_millis() as u64,
                message: format!("Erro de rede na Sólides: {}", e),
            },
        ),
    }
}

fn extract_attr(haystack: &str, prefix: &str) -> Option<String> {
    let idx = haystack.find(prefix)?;
    let rest = &haystack[idx + prefix.len()..];
    let end = rest.find('"')?;
    Some(rest[..end].trim().to_string())
}

fn extract_between(haystack: &str, start_tag: &str, end_tag: &str) -> Option<String> {
    let idx = haystack.find(start_tag)?;
    let rest = &haystack[idx + start_tag.len()..];
    let end = rest.find(end_tag)?;
    Some(rest[..end].to_string())
}

fn clean_html_snippet(raw: &str) -> String {
    let mut out = String::new();
    let mut inside_tag = false;
    for ch in raw.chars() {
        match ch {
            '<' => inside_tag = true,
            '>' => {
                inside_tag = false;
                out.push(' ');
            }
            _ if !inside_tag => out.push(ch),
            _ => {}
        }
    }
    decode_html_entities(&out)
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn decode_html_entities(s: &str) -> String {
    s.replace("&amp;", "&")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&nbsp;", " ")
}

fn urlencoding_simple(s: &str) -> String {
    s.trim().replace(' ', "%20")
}
