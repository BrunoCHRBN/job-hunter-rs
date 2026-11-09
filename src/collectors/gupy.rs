use anyhow::{Context, Result};
use reqwest::Client;
use serde::Deserialize;
use std::time::Duration;
use tracing::{debug, info, warn};

use crate::models::RawJobListing;

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub struct GupyApiResponse {
    pub data: Vec<GupyJobItem>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub struct GupyJobItem {
    pub id: serde_json::Value,
    pub name: Option<String>,
    pub description: Option<String>,
    #[serde(rename = "careerPageName")]
    pub career_page_name: Option<String>,
    pub city: Option<String>,
    pub state: Option<String>,
    pub country: Option<String>,
    #[serde(rename = "isRemoteWork")]
    pub is_remote_work: bool,
    #[serde(rename = "workplaceType")]
    pub workplace_type: Option<String>,
    #[serde(rename = "type")]
    pub job_type: Option<String>,
    #[serde(rename = "publishedDate")]
    pub published_date: Option<String>,
    #[serde(rename = "jobUrl")]
    pub job_url: Option<String>,
}

/// Remove tags HTML básicas e decodifica entidades HTML comuns preservando quebras de linha.
pub fn clean_html(raw: &str) -> String {
    if raw.trim().is_empty() {
        return String::new();
    }

    let pre_formatted = raw
        .replace("<br>", "\n")
        .replace("<br/>", "\n")
        .replace("<br />", "\n")
        .replace("</p>", "\n\n")
        .replace("</li>", "\n")
        .replace("<li>", "• ")
        .replace("</div>", "\n")
        .replace("</h1>", "\n")
        .replace("</h2>", "\n")
        .replace("</h3>", "\n");

    let mut out = String::with_capacity(pre_formatted.len());
    let mut inside_tag = false;

    for ch in pre_formatted.chars() {
        match ch {
            '<' => inside_tag = true,
            '>' => inside_tag = false,
            _ if !inside_tag => out.push(ch),
            _ => {}
        }
    }

    let decoded = out
        .replace("&nbsp;", " ")
        .replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&apos;", "'");

    let mut normalized_lines = Vec::new();
    let mut blank_streak = 0usize;

    for line in decoded.lines() {
        let trimmed = line.split_whitespace().collect::<Vec<_>>().join(" ");
        if trimmed.is_empty() {
            blank_streak += 1;
            if blank_streak <= 1 {
                normalized_lines.push(String::new());
            }
        } else {
            blank_streak = 0;
            normalized_lines.push(trimmed);
        }
    }

    normalized_lines.join("\n").trim().to_string()
}

fn value_to_id_string(val: &serde_json::Value) -> String {
    match val {
        serde_json::Value::Number(n) => n.to_string(),
        serde_json::Value::String(s) => s.clone(),
        _ => String::new(),
    }
}

/// Normaliza nomes de cidades brasileiras para o formato exato esperado pelo filtro `city=` da Gupy
/// (Title Case com acentuação correta, pois o filtro `city` da Gupy é case-sensitive).
pub fn normalize_city_name(raw: &str) -> String {
    let cleaned = raw
        .split(['-', '/'])
        .next()
        .unwrap_or(raw)
        .trim();

    if cleaned.is_empty() {
        return String::new();
    }

    let ascii_key = cleaned
        .to_lowercase()
        .replace(['á', 'à', 'â', 'ã'], "a")
        .replace(['é', 'ê'], "e")
        .replace('í', "i")
        .replace(['ó', 'ô', 'õ'], "o")
        .replace(['ú', 'ü'], "u")
        .replace('ç', "c");

    match ascii_key.as_str() {
        "araraquara" => return "Araraquara".to_string(),
        "sao carlos" => return "São Carlos".to_string(),
        "matao" => return "Matão".to_string(),
        "americo brasiliense" => return "Américo Brasiliense".to_string(),
        "ribeirao preto" => return "Ribeirão Preto".to_string(),
        "sao paulo" => return "São Paulo".to_string(),
        "sao jose do rio preto" => return "São José do Rio Preto".to_string(),
        "jaboticabal" => return "Jaboticabal".to_string(),
        "ibate" => return "Ibaté".to_string(),
        "bebedouro" => return "Bebedouro".to_string(),
        "sertãozinho" | "sertaozinho" => return "Sertãozinho".to_string(),
        "limeira" => return "Limeira".to_string(),
        "piracicaba" => return "Piracicaba".to_string(),
        "rio claro" => return "Rio Claro".to_string(),
        "campinas" => return "Campinas".to_string(),
        "bauru" => return "Bauru".to_string(),
        "jau" => return "Jaú".to_string(),
        "barretos" => return "Barretos".to_string(),
        "franca" => return "Franca".to_string(),
        "uberlandia" => return "Uberlândia".to_string(),
        "belo horizonte" => return "Belo Horizonte".to_string(),
        "curitiba" => return "Curitiba".to_string(),
        "florianopolis" => return "Florianópolis".to_string(),
        "porto alegre" => return "Porto Alegre".to_string(),
        "brasilia" => return "Brasília".to_string(),
        "goiania" => return "Goiânia".to_string(),
        _ => {}
    }

    let prepositions = ["de", "da", "do", "das", "dos", "e"];
    cleaned
        .split_whitespace()
        .enumerate()
        .map(|(idx, word)| {
            let lower = word.to_lowercase();
            if idx > 0 && prepositions.contains(&lower.as_str()) {
                lower
            } else {
                let mut chars = lower.chars();
                match chars.next() {
                    Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
                    None => String::new(),
                }
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// Converte uma string de filtro de localização (ex: "Araraquara", "Araraquara, São Carlos", "Araraquara e região")
/// em uma lista de cidades normalizadas para consulta na Gupy.
pub fn parse_target_cities(location_filter: Option<&str>) -> Vec<String> {
    let Some(raw) = location_filter else {
        return Vec::new();
    };
    let trimmed = raw.trim();
    if trimmed.is_empty()
        || trimmed.eq_ignore_ascii_case("Brasil")
        || trimmed.eq_ignore_ascii_case("Brazil")
        || trimmed.eq_ignore_ascii_case("Remoto")
    {
        return Vec::new();
    }

    let lower = trimmed.to_lowercase();
    if lower.contains("região") || lower.contains("regiao") {
        return vec![
            "Araraquara".to_string(),
            "São Carlos".to_string(),
            "Matão".to_string(),
            "Américo Brasiliense".to_string(),
        ];
    }

    trimmed
        .split([',', ';'])
        .map(normalize_city_name)
        .filter(|c| !c.is_empty())
        .collect()
}

fn strip_accents_lower(s: &str) -> String {
    s.to_lowercase()
        .replace(['á', 'à', 'â', 'ã'], "a")
        .replace(['é', 'ê'], "e")
        .replace('í', "i")
        .replace(['ó', 'ô', 'õ'], "o")
        .replace(['ú', 'ü'], "u")
        .replace('ç', "c")
}

fn extract_next_data_jobs(html: &str) -> Option<GupyApiResponse> {
    let marker = "<script id=\"__NEXT_DATA__\"";
    let start_idx = html.find(marker)?;
    let after_marker = &html[start_idx..];
    let gt_idx = after_marker.find('>')?;
    let json_start = &after_marker[gt_idx + 1..];
    let end_idx = json_start.find("</script>")?;
    let raw_json = &json_start[..end_idx];

    let root: serde_json::Value = serde_json::from_str(raw_json).ok()?;
    let job_list_val = root
        .get("props")?
        .get("pageProps")?
        .get("initialJobList")?
        .clone();

    serde_json::from_value::<GupyApiResponse>(job_list_val).ok()
}

async fn request_gupy_portal(
    client: &Client,
    query: &str,
    cities: &[String],
    remote_only: bool,
    effective_limit: usize,
) -> Result<GupyApiResponse> {
    let mut url = format!(
        "https://portal.gupy.io/job-search/term={}",
        urlencoding::encode(query.trim())
    );

    for city in cities {
        url.push_str(&format!("&city={}", urlencoding::encode(city)));
    }

    if remote_only {
        url.push_str("&workplaceTypes=remote");
    }

    debug!("Consultando Gupy Portal SSR: {}", url);

    let portal_resp = client
        .get(&url)
        .header(
            "User-Agent",
            "Mozilla/5.0 (X11; Linux x86_64) JobHunterRS/0.1",
        )
        .header("Accept", "text/html,application/xhtml+xml,application/json")
        .send()
        .await
        .context("Falha de rede ao consultar portal.gupy.io")?;

    if !portal_resp.status().is_success() {
        warn!(
            "Portal Gupy retornou status não-sucesso ({})",
            portal_resp.status()
        );
        return Ok(GupyApiResponse::default());
    }

    let html = portal_resp.text().await.unwrap_or_default();
    if let Some(mut extracted) = extract_next_data_jobs(&html) {
        extracted.data.truncate(effective_limit);
        return Ok(extracted);
    }

    Ok(GupyApiResponse::default())
}

pub async fn fetch_gupy_jobs(
    query: &str,
    location_filter: Option<&str>,
    limit: usize,
    remote_only: bool,
) -> Result<Vec<RawJobListing>> {
    let effective_limit = limit.clamp(1, 100);
    let target_cities = parse_target_cities(location_filter);

    let client = Client::builder()
        .timeout(Duration::from_secs(20))
        .build()
        .context("Falha ao criar cliente HTTP para a Gupy")?;

    let payload = request_gupy_portal(
        &client,
        query,
        &target_cities,
        remote_only,
        effective_limit,
    )
    .await?;

    let mut listings = Vec::new();

    for item in payload.data {
        let is_remote = item.is_remote_work
            || item
                .workplace_type
                .as_deref()
                .map(|w| w.eq_ignore_ascii_case("remote"))
                .unwrap_or(false);

        if remote_only && !is_remote {
            continue;
        }

        let city = item.city.unwrap_or_default().trim().to_string();
        let state = item.state.unwrap_or_default().trim().to_string();
        let country = item
            .country
            .unwrap_or_else(|| "Brasil".to_string())
            .trim()
            .to_string();

        /* Se o usuário especificou cidade(s) e não pediu apenas remoto, garante que a vaga pertence à cidade solicitada */
        if !target_cities.is_empty() && !remote_only {
            let item_city_norm = strip_accents_lower(&city);
            let matches_city = target_cities
                .iter()
                .any(|tc| strip_accents_lower(tc) == item_city_norm);
            if !matches_city && !is_remote {
                continue;
            }
        }

        let location = match (city.is_empty(), state.is_empty(), is_remote) {
            (false, false, true) => format!("{} - {} (Remoto)", city, state),
            (false, false, false) => format!("{} - {}", city, state),
            (false, true, true) => format!("{} (Remoto)", city),
            (false, true, false) => city.clone(),
            (true, false, true) => format!("{} (Remoto)", state),
            (true, false, false) => state.clone(),
            (true, true, true) => "Remoto".to_string(),
            (true, true, false) => country,
        };

        let external_id = value_to_id_string(&item.id);
        let title = item
            .name
            .unwrap_or_else(|| "Vaga sem título".to_string())
            .trim()
            .to_string();
        let company = item
            .career_page_name
            .unwrap_or_else(|| "Empresa Gupy".to_string())
            .trim()
            .to_string();
        let job_url = item.job_url.unwrap_or_default().trim().to_string();

        let raw_html = item.description.unwrap_or_default();
        let cleaned_desc = clean_html(&raw_html);
        let workplace_hint = item.workplace_type.unwrap_or_default();
        let contract_hint = item.job_type.unwrap_or_default();

        let enriched_raw_description = format!(
            "[Metadados Gupy: workplaceType={}, isRemoteWork={}, type={}, local={}]\n\n{}",
            workplace_hint, is_remote, contract_hint, location, cleaned_desc
        );

        listings.push(RawJobListing {
            external_id: if external_id.is_empty() {
                job_url.clone()
            } else {
                external_id
            },
            source: "gupy".to_string(),
            title,
            company,
            location,
            url: job_url,
            raw_description: enriched_raw_description,
            published_at: item.published_date,
        });
    }

    info!(
        "Coletor Gupy retornou {} vagas para '{}' (cidades: {:?}, remoto: {})",
        listings.len(),
        query,
        target_cities,
        remote_only
    );
    Ok(listings)
}
