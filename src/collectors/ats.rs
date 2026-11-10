use std::time::{Duration, Instant};

use reqwest::Client;
use serde_json::Value;

use crate::collectors::brasil::{SourceDiagnosticReport, SourceDiagnosticStatus};
use crate::collectors::gupy;
use crate::models::RawJobListing;
use crate::normalizer::{normalize_company_name, strip_accents};

/// Coleta vagas diretamente da página de carreira / provedor ATS de uma empresa.
pub async fn fetch_company_ats_jobs(
    ats_provider: &str,
    portal_id: &str,
    career_url: &str,
    company_name: &str,
    query_filter: Option<&str>,
    limit: usize,
) -> (Vec<RawJobListing>, SourceDiagnosticReport) {
    let provider = ats_provider.trim().to_lowercase();
    match provider.as_str() {
        "greenhouse" => fetch_greenhouse_board(portal_id, company_name, query_filter, limit).await,
        "lever" => fetch_lever_board(portal_id, company_name, query_filter, limit).await,
        "ashby" => fetch_ashby_board(portal_id, company_name, query_filter, limit).await,
        "gupy" => fetch_gupy_company_board(portal_id, company_name, query_filter, limit).await,
        _ => {
            fetch_generic_career_page_jsonld(
                career_url,
                company_name,
                &provider,
                query_filter,
                limit,
            )
            .await
        }
    }
}

/// Coleta vagas via API pública oficial da Greenhouse (`boards-api.greenhouse.io`).
pub async fn fetch_greenhouse_board(
    portal_id: &str,
    company_name: &str,
    query_filter: Option<&str>,
    limit: usize,
) -> (Vec<RawJobListing>, SourceDiagnosticReport) {
    let started = Instant::now();
    let slug = portal_id.trim();
    let source_id = format!("greenhouse:{}", slug);
    let url = format!(
        "https://boards-api.greenhouse.io/v1/boards/{}/jobs?content=true",
        slug
    );

    let client = match Client::builder().timeout(Duration::from_secs(20)).build() {
        Ok(c) => c,
        Err(e) => {
            return error_report(&source_id, "Greenhouse ATS", started, e.to_string());
        }
    };

    let resp = match client.get(&url).send().await {
        Ok(r) => r,
        Err(e) => {
            return net_err_report(&source_id, "Greenhouse ATS", started, e);
        }
    };

    if resp.status().as_u16() == 403 || resp.status().as_u16() == 429 {
        return blocked_report(&source_id, "Greenhouse ATS", started, resp.status().as_u16());
    }
    if !resp.status().is_success() {
        return error_report(
            &source_id,
            "Greenhouse ATS",
            started,
            format!("HTTP {} em {}", resp.status(), url),
        );
    }

    let json: Value = match resp.json().await {
        Ok(v) => v,
        Err(e) => {
            return error_report(&source_id, "Greenhouse ATS", started, e.to_string());
        }
    };

    let mut out = Vec::new();
    let needle = query_filter.map(strip_accents).unwrap_or_default();

    if let Some(arr) = json["jobs"].as_array() {
        for item in arr {
            if out.len() >= limit {
                break;
            }
            let title = item["title"].as_str().unwrap_or("").trim().to_string();
            if title.is_empty() {
                continue;
            }
            let loc = item["location"]["name"]
                .as_str()
                .unwrap_or("Brasil")
                .trim()
                .to_string();

            if !needle.is_empty() {
                let hay = strip_accents(&format!("{} {}", title, loc));
                if !hay.contains(&needle) {
                    continue;
                }
            }

            let ext_id = item["id"]
                .as_i64()
                .map(|n| n.to_string())
                .or_else(|| item["id"].as_str().map(|s| s.to_string()))
                .unwrap_or_else(|| title.clone());
            let abs_url = item["absolute_url"].as_str().unwrap_or("").to_string();
            let content = item["content"].as_str().unwrap_or(&title).to_string();
            let published_at = item["updated_at"].as_str().map(|s| s.to_string());

            out.push(RawJobListing {
                external_id: format!("gh_{}_{}", slug, ext_id),
                source: source_id.clone(),
                title,
                company: company_name.to_string(),
                location: loc,
                url: abs_url,
                raw_description: content,
                published_at,
            });
        }
    }

    finalize_report(&source_id, "Greenhouse ATS", started, out)
}

/// Coleta vagas via API pública oficial da Lever (`api.lever.co/v0/postings/{portal_id}?mode=json`).
pub async fn fetch_lever_board(
    portal_id: &str,
    company_name: &str,
    query_filter: Option<&str>,
    limit: usize,
) -> (Vec<RawJobListing>, SourceDiagnosticReport) {
    let started = Instant::now();
    let slug = portal_id.trim();
    let source_id = format!("lever:{}", slug);
    let url = format!("https://api.lever.co/v0/postings/{}?mode=json", slug);

    let client = match Client::builder().timeout(Duration::from_secs(20)).build() {
        Ok(c) => c,
        Err(e) => return error_report(&source_id, "Lever ATS", started, e.to_string()),
    };

    let resp = match client.get(&url).send().await {
        Ok(r) => r,
        Err(e) => return net_err_report(&source_id, "Lever ATS", started, e),
    };

    if resp.status().as_u16() == 403 || resp.status().as_u16() == 429 {
        return blocked_report(&source_id, "Lever ATS", started, resp.status().as_u16());
    }
    if !resp.status().is_success() {
        return error_report(
            &source_id,
            "Lever ATS",
            started,
            format!("HTTP {} em {}", resp.status(), url),
        );
    }

    let json: Value = match resp.json().await {
        Ok(v) => v,
        Err(e) => return error_report(&source_id, "Lever ATS", started, e.to_string()),
    };

    let mut out = Vec::new();
    let needle = query_filter.map(strip_accents).unwrap_or_default();

    if let Some(arr) = json.as_array() {
        for item in arr {
            if out.len() >= limit {
                break;
            }
            let title = item["text"].as_str().unwrap_or("").trim().to_string();
            if title.is_empty() {
                continue;
            }
            let loc = item["categories"]["location"]
                .as_str()
                .unwrap_or("Brasil")
                .trim()
                .to_string();

            if !needle.is_empty() {
                let hay = strip_accents(&format!("{} {}", title, loc));
                if !hay.contains(&needle) {
                    continue;
                }
            }

            let ext_id = item["id"].as_str().unwrap_or(&title).to_string();
            let hosted_url = item["hostedUrl"].as_str().unwrap_or("").to_string();
            let desc = item["descriptionPlain"]
                .as_str()
                .or_else(|| item["description"].as_str())
                .unwrap_or(&title)
                .to_string();

            out.push(RawJobListing {
                external_id: format!("lever_{}_{}", slug, ext_id),
                source: source_id.clone(),
                title,
                company: company_name.to_string(),
                location: loc,
                url: hosted_url,
                raw_description: desc,
                published_at: None,
            });
        }
    }

    finalize_report(&source_id, "Lever ATS", started, out)
}

/// Coleta vagas via API pública oficial da Ashby (`api.ashbyhq.com/posting-api/job-board/{portal_id}`).
pub async fn fetch_ashby_board(
    portal_id: &str,
    company_name: &str,
    query_filter: Option<&str>,
    limit: usize,
) -> (Vec<RawJobListing>, SourceDiagnosticReport) {
    let started = Instant::now();
    let slug = portal_id.trim();
    let source_id = format!("ashby:{}", slug);
    let url = format!("https://api.ashbyhq.com/posting-api/job-board/{}", slug);

    let client = match Client::builder().timeout(Duration::from_secs(20)).build() {
        Ok(c) => c,
        Err(e) => return error_report(&source_id, "Ashby ATS", started, e.to_string()),
    };

    let resp = match client.get(&url).send().await {
        Ok(r) => r,
        Err(e) => return net_err_report(&source_id, "Ashby ATS", started, e),
    };

    if resp.status().as_u16() == 403 || resp.status().as_u16() == 429 {
        return blocked_report(&source_id, "Ashby ATS", started, resp.status().as_u16());
    }
    if !resp.status().is_success() {
        return error_report(
            &source_id,
            "Ashby ATS",
            started,
            format!("HTTP {} em {}", resp.status(), url),
        );
    }

    let json: Value = match resp.json().await {
        Ok(v) => v,
        Err(e) => return error_report(&source_id, "Ashby ATS", started, e.to_string()),
    };

    let mut out = Vec::new();
    let needle = query_filter.map(strip_accents).unwrap_or_default();

    if let Some(arr) = json["jobs"].as_array() {
        for item in arr {
            if out.len() >= limit {
                break;
            }
            let title = item["title"].as_str().unwrap_or("").trim().to_string();
            if title.is_empty() {
                continue;
            }
            let loc = item["location"]
                .as_str()
                .unwrap_or("Brasil")
                .trim()
                .to_string();

            if !needle.is_empty() {
                let hay = strip_accents(&format!("{} {}", title, loc));
                if !hay.contains(&needle) {
                    continue;
                }
            }

            let ext_id = item["id"].as_str().unwrap_or(&title).to_string();
            let job_url = item["jobUrl"].as_str().unwrap_or("").to_string();
            let desc = item["descriptionPlain"]
                .as_str()
                .or_else(|| item["descriptionHtml"].as_str())
                .unwrap_or(&title)
                .to_string();
            let pub_at = item["publishedAt"].as_str().map(|s| s.to_string());

            out.push(RawJobListing {
                external_id: format!("ashby_{}_{}", slug, ext_id),
                source: source_id.clone(),
                title,
                company: company_name.to_string(),
                location: loc,
                url: job_url,
                raw_description: desc,
                published_at: pub_at,
            });
        }
    }

    finalize_report(&source_id, "Ashby ATS", started, out)
}

/// Coleta vagas corporativas na Gupy filtrando pela empresa alvo.
pub async fn fetch_gupy_company_board(
    portal_id: &str,
    company_name: &str,
    query_filter: Option<&str>,
    limit: usize,
) -> (Vec<RawJobListing>, SourceDiagnosticReport) {
    let started = Instant::now();
    let slug = portal_id.trim();
    let source_id = format!("gupy:{}", slug);

    let search_term = match query_filter {
        Some(q) if !q.trim().is_empty() => format!("{} {}", company_name, q),
        _ => company_name.to_string(),
    };

    match gupy::fetch_gupy_jobs(&search_term, None, limit * 2, false, None).await {
        Ok(raw_list) => {
            let target_norm = normalize_company_name(company_name);
            let slug_norm = strip_accents(slug);
            let filtered: Vec<RawJobListing> = raw_list
                .into_iter()
                .filter(|j| {
                    let j_norm = normalize_company_name(&j.company);
                    let u_norm = strip_accents(&j.url);
                    j_norm.contains(&target_norm)
                        || target_norm.contains(&j_norm)
                        || (!slug_norm.is_empty() && u_norm.contains(&slug_norm))
                })
                .take(limit)
                .map(|mut j| {
                    j.source = source_id.clone();
                    j
                })
                .collect();

            finalize_report(&source_id, "Gupy Corporate Portal", started, filtered)
        }
        Err(e) => error_report(&source_id, "Gupy Corporate Portal", started, e.to_string()),
    }
}

/// Coleta páginas de carreira genéricas ou Workday extraindo blocos `<script type="application/ld+json">` do tipo `JobPosting`.
pub async fn fetch_generic_career_page_jsonld(
    career_url: &str,
    company_name: &str,
    provider_tag: &str,
    query_filter: Option<&str>,
    limit: usize,
) -> (Vec<RawJobListing>, SourceDiagnosticReport) {
    let started = Instant::now();
    let source_id = format!(
        "{}:{}",
        if provider_tag.is_empty() {
            "careers"
        } else {
            provider_tag
        },
        normalize_company_name(company_name).replace(' ', "_")
    );

    if career_url.trim().is_empty() {
        return error_report(
            &source_id,
            "Página de Carreira",
            started,
            "URL da página de carreira vazia".to_string(),
        );
    }

    let client = match Client::builder()
        .timeout(Duration::from_secs(20))
        .user_agent("Mozilla/5.0 (X11; Linux x86_64) Chrome/124.0.0.0 Safari/537.36")
        .build()
    {
        Ok(c) => c,
        Err(e) => return error_report(&source_id, "Página de Carreira", started, e.to_string()),
    };

    let resp = match client.get(career_url).send().await {
        Ok(r) => r,
        Err(e) => return net_err_report(&source_id, "Página de Carreira", started, e),
    };

    if resp.status().as_u16() == 403 || resp.status().as_u16() == 429 {
        return blocked_report(
            &source_id,
            "Página de Carreira",
            started,
            resp.status().as_u16(),
        );
    }
    if !resp.status().is_success() {
        return error_report(
            &source_id,
            "Página de Carreira",
            started,
            format!("HTTP {} ao acessar {}", resp.status(), career_url),
        );
    }

    let html = resp.text().await.unwrap_or_default();
    let mut out = Vec::new();
    let needle = query_filter.map(strip_accents).unwrap_or_default();

    for segment in html.split("application/ld+json").skip(1) {
        let Some(start_idx) = segment.find('>') else {
            continue;
        };
        let rest = &segment[start_idx + 1..];
        let Some(end_idx) = rest.find("</script>") else {
            continue;
        };
        let raw_json = rest[..end_idx].trim();
        if let Ok(val) = serde_json::from_str::<Value>(raw_json) {
            extract_jsonld_postings(
                &val,
                company_name,
                career_url,
                &source_id,
                &needle,
                limit,
                &mut out,
            );
        }
    }

    finalize_report(&source_id, "Página de Carreira", started, out)
}

fn extract_jsonld_postings(
    val: &Value,
    company_name: &str,
    fallback_url: &str,
    source_id: &str,
    needle: &str,
    limit: usize,
    out: &mut Vec<RawJobListing>,
) {
    if out.len() >= limit {
        return;
    }
    if let Some(arr) = val.as_array() {
        for item in arr {
            extract_jsonld_postings(
                item,
                company_name,
                fallback_url,
                source_id,
                needle,
                limit,
                out,
            );
        }
        return;
    }
    if val["@type"].as_str() == Some("JobPosting") {
        let title = val["title"].as_str().unwrap_or("").trim().to_string();
        if title.is_empty() {
            return;
        }
        let loc = val["jobLocation"]["address"]["addressLocality"]
            .as_str()
            .unwrap_or("Brasil")
            .to_string();
        if !needle.is_empty() && !strip_accents(&format!("{} {}", title, loc)).contains(needle) {
            return;
        }
        let desc = val["description"].as_str().unwrap_or(&title).to_string();
        let url = val["url"].as_str().unwrap_or(fallback_url).to_string();
        let ext_id = val["identifier"]["value"]
            .as_str()
            .unwrap_or(&title)
            .to_string();

        out.push(RawJobListing {
            external_id: ext_id,
            source: source_id.to_string(),
            title,
            company: company_name.to_string(),
            location: loc,
            url,
            raw_description: desc,
            published_at: val["datePosted"].as_str().map(|s| s.to_string()),
        });
    }
}

fn finalize_report(
    source_id: &str,
    display_name: &str,
    started: Instant,
    jobs: Vec<RawJobListing>,
) -> (Vec<RawJobListing>, SourceDiagnosticReport) {
    let dur = started.elapsed().as_millis() as u64;
    let status = if jobs.is_empty() {
        SourceDiagnosticStatus::Empty
    } else {
        SourceDiagnosticStatus::Success
    };
    let msg = if jobs.is_empty() {
        format!("Nenhuma vaga aberta encontrada em {}", display_name)
    } else {
        format!("{} vagas obtidas via {}", jobs.len(), display_name)
    };
    (
        jobs.clone(),
        SourceDiagnosticReport {
            source_id: source_id.to_string(),
            display_name: display_name.to_string(),
            status,
            count: jobs.len(),
            duration_ms: dur,
            message: msg,
        },
    )
}

fn error_report(
    source_id: &str,
    display_name: &str,
    started: Instant,
    msg: String,
) -> (Vec<RawJobListing>, SourceDiagnosticReport) {
    (
        Vec::new(),
        SourceDiagnosticReport {
            source_id: source_id.to_string(),
            display_name: display_name.to_string(),
            status: SourceDiagnosticStatus::Error,
            count: 0,
            duration_ms: started.elapsed().as_millis() as u64,
            message: msg,
        },
    )
}

fn blocked_report(
    source_id: &str,
    display_name: &str,
    started: Instant,
    http_code: u16,
) -> (Vec<RawJobListing>, SourceDiagnosticReport) {
    (
        Vec::new(),
        SourceDiagnosticReport {
            source_id: source_id.to_string(),
            display_name: display_name.to_string(),
            status: SourceDiagnosticStatus::Blocked,
            count: 0,
            duration_ms: started.elapsed().as_millis() as u64,
            message: format!("Acesso limitado (HTTP {}) em {}", http_code, display_name),
        },
    )
}

fn net_err_report(
    source_id: &str,
    display_name: &str,
    started: Instant,
    err: reqwest::Error,
) -> (Vec<RawJobListing>, SourceDiagnosticReport) {
    let status = if err.is_timeout() {
        SourceDiagnosticStatus::Timeout
    } else {
        SourceDiagnosticStatus::Error
    };
    (
        Vec::new(),
        SourceDiagnosticReport {
            source_id: source_id.to_string(),
            display_name: display_name.to_string(),
            status,
            count: 0,
            duration_ms: started.elapsed().as_millis() as u64,
            message: format!("Falha de rede em {}: {}", display_name, err),
        },
    )
}
