use std::collections::HashSet;

/// Remove acentos e converte para minúsculas.
pub fn strip_accents(s: &str) -> String {
    s.to_lowercase()
        .chars()
        .map(|c| match c {
            'á' | 'à' | 'â' | 'ã' | 'ä' => 'a',
            'é' | 'è' | 'ê' | 'ë' => 'e',
            'í' | 'ì' | 'î' | 'ï' => 'i',
            'ó' | 'ò' | 'ô' | 'õ' | 'ö' => 'o',
            'ú' | 'ù' | 'û' | 'ü' => 'u',
            'ç' => 'c',
            'ñ' => 'n',
            other => other,
        })
        .collect()
}

/// Normaliza o nome de uma empresa removendo sufixos societários, pontuação e acentuação.
pub fn normalize_company_name(name: &str) -> String {
    let raw = strip_accents(name);
    let cleaned: String = raw
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c.is_whitespace() {
                c
            } else {
                ' '
            }
        })
        .collect();

    let legal_suffixes = [
        "ltda", "eireli", "me", "epp", "inc", "llc", "corp", "holding", "s a", "sa",
    ];

    let mut tokens: Vec<&str> = cleaned.split_whitespace().collect();

    // Remove sufixos societários repetidos no final
    while tokens.len() > 1 {
        let last = *tokens.last().unwrap();
        if legal_suffixes.contains(&last) {
            tokens.pop();
            continue;
        }
        if tokens.len() >= 2 {
            let last_two = format!("{} {}", tokens[tokens.len() - 2], tokens[tokens.len() - 1]);
            if last_two == "s a" || last_two == "do brasil" {
                tokens.pop();
                tokens.pop();
                continue;
            }
        }
        if last == "brasil" && tokens.len() >= 2 {
            tokens.pop();
            continue;
        }
        break;
    }

    tokens.join(" ")
}

/// Normaliza o título de uma vaga removendo marcadores de gênero, sufixos de modalidade
/// e padronizando abreviações comuns.
pub fn normalize_job_title(title: &str) -> String {
    let mut s = strip_accents(title);

    for marker in [
        "(a)",
        "(o)",
        "(as)",
        "(os)",
        "(m/f)",
        "m/f",
        "(pcd)",
        "(hibrido)",
        "(remoto)",
        "(presencial)",
        "100% remoto",
        "home office",
        "vaga afirmativa",
    ] {
        s = s.replace(marker, " ");
    }

    let cleaned: String = s
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c.is_whitespace() {
                c
            } else {
                ' '
            }
        })
        .collect();

    let mut normalized_tokens = Vec::new();
    for tok in cleaned.split_whitespace() {
        let mapped = match tok {
            "jr" => "junior",
            "pl" => "pleno",
            "sr" => "senior",
            "dev" => "desenvolvedor",
            "eng" => "engenheiro",
            "coord" => "coordenador",
            "sup" => "supervisor",
            "aux" => "auxiliar",
            "op" => "operador",
            "tec" => "tecnico",
            "remoto" | "presencial" | "hibrido" => continue,
            other => other,
        };
        normalized_tokens.push(mapped);
    }

    normalized_tokens.join(" ")
}

/// Padroniza a localização para formato canônico (ex: `araraquara-sp` ou `remoto`).
pub fn normalize_location(loc: &str) -> String {
    let s = strip_accents(loc.trim());
    if s.is_empty()
        || s == "n/d"
        || s == "local n/d"
        || s == "nao informado"
        || s == "brasil"
        || s == "brazil"
    {
        return "brasil".to_string();
    }

    if s.contains("remoto")
        || s.contains("remote")
        || s.contains("home office")
        || s.contains("todo o brasil")
    {
        return "remoto".to_string();
    }

    let state_map = [
        ("sao paulo", "sp"),
        ("rio de janeiro", "rj"),
        ("minas gerais", "mg"),
        ("parana", "pr"),
        ("rio grande do sul", "rs"),
        ("santa catarina", "sc"),
        ("bahia", "ba"),
        ("pernambuco", "pe"),
        ("ceara", "ce"),
        ("goias", "go"),
        ("distrito federal", "df"),
        ("espirito santo", "es"),
        ("mato grosso do sul", "ms"),
        ("mato grosso", "mt"),
    ];

    let mut work = s.replace("brazil", "").replace("brasil", "");
    for sep in ['/', '-', ','] {
        work = work.replace(sep, "|");
    }

    let parts: Vec<String> = work
        .split('|')
        .map(|p| p.split_whitespace().collect::<Vec<_>>().join(" "))
        .filter(|p| !p.is_empty() && p != "efetivo")
        .collect();

    if parts.is_empty() {
        return "brasil".to_string();
    }

    let city = &parts[0];
    if parts.len() >= 2 {
        let mut st = parts[1].clone();
        for (full, uf) in state_map {
            if st == full {
                st = uf.to_string();
                break;
            }
        }
        if st.len() == 2 {
            return format!("{}-{}", city, st);
        }
    }

    city.clone()
}

/// Padroniza a modalidade de trabalho (`Remoto`, `Hibrido`, `Presencial`, `Nao Informado`).
pub fn normalize_work_model(
    model: &str,
    location: &str,
    title: &str,
    description: &str,
) -> String {
    let m = strip_accents(model);
    if m.contains("remot") || m.contains("home") || m.contains("teletrabalho") {
        return "Remoto".to_string();
    }
    if m.contains("hibrid") || m.contains("hybrid") {
        return "Hibrido".to_string();
    }
    if m.contains("presencial") || m.contains("onsite") || m.contains("on-site") {
        return "Presencial".to_string();
    }

    let combined = strip_accents(&format!("{} {} {}", location, title, description));
    if combined.contains("100% remoto")
        || combined.contains("trabalho remoto")
        || combined.contains("vaga remota")
        || strip_accents(location).contains("remoto")
    {
        return "Remoto".to_string();
    }
    if combined.contains("modelo hibrido") || combined.contains("regime hibrido") {
        return "Hibrido".to_string();
    }
    if combined.contains("100% presencial") || combined.contains("trabalho presencial") {
        return "Presencial".to_string();
    }

    "Nao Informado".to_string()
}

/// Padroniza a senioridade (`Estagio`, `Junior`, `Pleno`, `Senior`, `Especialista`, `Lideranca`, `Operacional`, `Nao Informado`).
pub fn normalize_seniority(sen: &str, title: &str) -> String {
    let combined = strip_accents(&format!("{} {}", sen, title));
    if combined.contains("estagi") || combined.contains("intern") || combined.contains("trainee") {
        return "Estagio".to_string();
    }
    if combined.contains("diretor")
        || combined.contains("gerente")
        || combined.contains("coordenador")
        || combined.contains("supervisor")
        || combined.contains("tech lead")
        || combined.contains("lider")
    {
        return "Lideranca".to_string();
    }
    if combined.contains("especialista") || combined.contains("staff") || combined.contains("principal") {
        return "Especialista".to_string();
    }
    if combined.contains("senior") || combined.contains(" sr") || combined.ends_with(" iii") {
        return "Senior".to_string();
    }
    if combined.contains("pleno") || combined.contains(" pl") || combined.ends_with(" ii") {
        return "Pleno".to_string();
    }
    if combined.contains("junior") || combined.contains(" jr") || combined.ends_with(" i") {
        return "Junior".to_string();
    }
    if combined.contains("operador")
        || combined.contains("auxiliar")
        || combined.contains("assistente")
        || combined.contains("ajudante")
        || combined.contains("tecnico")
    {
        return "Operacional".to_string();
    }
    "Nao Informado".to_string()
}

/// Padroniza o tipo de contrato (`CLT`, `PJ`, `Estagio`, `Temporario`, `Nao Informado`).
pub fn normalize_contract_type(ct: &str, title: &str, description: &str) -> String {
    let combined = strip_accents(&format!("{} {} {}", ct, title, description));
    if combined.contains("estagio") || combined.contains("estagiario") {
        return "Estagio".to_string();
    }
    if combined.contains("pj") || combined.contains("pessoa juridica") {
        return "PJ".to_string();
    }
    if combined.contains("temporari") {
        return "Temporario".to_string();
    }
    if combined.contains("clt") || combined.contains("efetivo") {
        return "CLT".to_string();
    }
    "Nao Informado".to_string()
}

/// Normaliza uma URL removendo parâmetros de rastreamento (`utm_*`, `trk`, `refId`, etc.) e barras finais.
pub fn canonicalize_url(raw_url: &str) -> String {
    let trimmed = raw_url.trim();
    if trimmed.is_empty() {
        return String::new();
    }

    let without_frag = trimmed.split('#').next().unwrap_or(trimmed);
    let mut parts = without_frag.splitn(2, '?');
    let base = parts.next().unwrap_or("").trim_end_matches('/');
    let query_opt = parts.next();

    let tracking_prefixes = [
        "utm_",
        "trk",
        "refid",
        "trackingid",
        "fbclid",
        "gclid",
        "gh_src",
        "source",
    ];

    if let Some(query) = query_opt {
        let kept: Vec<&str> = query
            .split('&')
            .filter(|kv| {
                let key = kv.split('=').next().unwrap_or("").to_lowercase();
                !key.is_empty()
                    && !tracking_prefixes.iter().any(|pref| key.starts_with(pref))
            })
            .collect();
        if !kept.is_empty() {
            return format!("{}?{}", base, kept.join("&"));
        }
    }

    base.to_string()
}

/// Extrai apenas a cidade-base canônica (ex: de 'araraquara-sp' ou 'araraquara, brazil' retorna 'araraquara').
pub fn extract_canonical_city(loc: &str) -> String {
    let norm = normalize_location(loc);
    if norm == "remoto" || norm == "brasil" {
        return norm;
    }
    norm.split('-').next().unwrap_or("").trim().to_string()
}

/// Verifica se duas localizações são geograficamente compatíveis (mesma cidade, região ou modalidade).
pub fn locations_are_compatible(loc_a: &str, loc_b: &str) -> bool {
    let norm_a = normalize_location(loc_a);
    let norm_b = normalize_location(loc_b);

    if norm_a == norm_b || norm_a == "brasil" || norm_b == "brasil" || norm_a == "remoto" || norm_b == "remoto" {
        return true;
    }

    let city_a = extract_canonical_city(loc_a);
    let city_b = extract_canonical_city(loc_b);

    if !city_a.is_empty() && city_a == city_b {
        return true;
    }

    false
}

/// Calcula o hash BLAKE3 aprimorado com campos normalizados (empresa, título e cidade canônica).
pub fn compute_enhanced_dedup_hash(company: &str, title: &str, location: &str) -> String {
    let norm_comp = normalize_company_name(company);
    let norm_title = normalize_job_title(title);
    let canon_city = extract_canonical_city(location);
    let payload = format!("{}|{}|{}", norm_comp, norm_title, canon_city);
    blake3::hash(payload.as_bytes()).to_hex().to_string()
}

/// Verifica se dois títulos de vaga (da mesma empresa/localidade) são quase idênticos
/// usando similaridade de Jaccard por tokens significativos ou subconjunto direto de cargo.
pub fn titles_are_near_duplicates(title_a: &str, title_b: &str) -> bool {
    let norm_a = normalize_job_title(title_a);
    let norm_b = normalize_job_title(title_b);

    if norm_a.is_empty() || norm_b.is_empty() {
        return false;
    }
    if norm_a == norm_b {
        return true;
    }

    let stopwords: HashSet<&str> = [
        "de", "da", "do", "das", "dos", "em", "para", "e", "com", "na", "no",
        "centro", "loja", "filial", "unidade", "vaga", "oportunidade",
    ]
    .into_iter()
    .collect();

    let tokens_a: HashSet<&str> = norm_a
        .split_whitespace()
        .filter(|t| !stopwords.contains(t))
        .collect();
    let tokens_b: HashSet<&str> = norm_b
        .split_whitespace()
        .filter(|t| !stopwords.contains(t))
        .collect();

    if tokens_a.is_empty() || tokens_b.is_empty() {
        return false;
    }

    let intersection = tokens_a.intersection(&tokens_b).count();
    let union = tokens_a.union(&tokens_b).count();

    if union == 0 {
        return false;
    }

    let jaccard = intersection as f64 / union as f64;
    if jaccard >= 0.70 {
        return true;
    }

    // Se um conjunto é subconjunto completo do outro e tem pelo menos 2 tokens nucleares
    // (ex: 'assistente administrativo' contido em 'assistente administrativo aradoces cia')
    let min_len = tokens_a.len().min(tokens_b.len());
    let max_len = tokens_a.len().max(tokens_b.len());
    if min_len >= 2 && intersection == min_len && (min_len as f64 / max_len as f64) >= 0.40 {
        return true;
    }

    false
}
