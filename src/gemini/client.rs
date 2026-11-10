use std::time::{Duration, Instant};

use anyhow::{anyhow, bail, Context, Result};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tracing::{debug, warn};

use crate::db::Database;
use crate::gemini::prompts::{
    build_batch_extraction_user_prompt, build_match_analysis_user_prompt_with_profile,
    BATCH_EXTRACTION_SYSTEM_PROMPT, MATCH_ANALYSIS_SYSTEM_PROMPT,
};
use crate::models::{
    BatchExtractionResponse, CandidateProfile, ExtractedJobMetadata, FitAxes, FitAxis, JobRecord,
    MatchAnalysisResult, RawJobListing,
};

const GEMINI_BASE_URL: &str = "https://generativelanguage.googleapis.com/v1beta/models";
const BATCH_EXTRACTION_MODELS: &[&str] = &[
    "gemini-3.1-flash-lite",
    "gemini-3-flash-preview",
    "gemini-flash-lite-latest",
    "gemini-3.5-flash-lite",
];
const MATCH_ANALYSIS_MODELS: &[&str] = &[
    "gemini-3-flash-preview",
    "gemini-3.1-flash-lite",
    "gemini-flash-lite-latest",
    "gemini-3.5-flash",
];
const RESUME_STRUCTURE_MODELS: &[&str] = &[
    "gemini-3-flash-preview",
    "gemini-3.1-flash-lite",
    "gemini-flash-lite-latest",
];
const EMBEDDING_MODELS: &[&str] = &[
    "text-embedding-004",
    "gemini-embedding-001",
    "gemini-embedding-2",
];

#[derive(Debug, Clone)]
pub struct GeminiCallOutcome {
    pub text: String,
    pub model_used: String,
    pub fallback_used: bool,
    pub attempts: i64,
    pub prompt_tokens: i64,
    pub completion_tokens: i64,
    pub duration_ms: i64,
}

#[derive(Debug, Clone)]
pub struct GeminiClient {
    http: Client,
    api_key: String,
    db: Option<Database>,
}

impl GeminiClient {
    pub fn new(api_key: impl Into<String>) -> Result<Self> {
        let http = Client::builder()
            .timeout(Duration::from_secs(18))
            .connect_timeout(Duration::from_secs(6))
            .build()
            .context("Falha ao inicializar cliente HTTP reqwest para o Gemini")?;

        Ok(Self {
            http,
            api_key: api_key.into(),
            db: None,
        })
    }

    pub fn with_db(mut self, db: Database) -> Self {
        self.db = Some(db);
        self
    }

    async fn call_generate_content_with_candidates(
        &self,
        operation_type: &str,
        records_count: usize,
        candidates: &[&str],
        system_instruction: &str,
        user_text: &str,
    ) -> Result<String> {
        let cache_key = compute_ai_cache_key(operation_type, system_instruction, user_text);

        if let Some(ref db) = self.db {
            if let Ok(Some(cached_json)) = db.get_ai_cache(&cache_key).await {
                let _ = db
                    .record_ai_operation(
                        None,
                        operation_type,
                        "cache:blake3",
                        false,
                        0,
                        "CACHE_HIT",
                        records_count,
                        0,
                        0,
                        0,
                        None,
                    )
                    .await;
                return Ok(cached_json);
            }
        }

        let started = Instant::now();
        let mut last_err = None;
        let mut attempts = 0_i64;

        for (idx, &model) in candidates.iter().enumerate() {
            let mut retry_count = 0;
            const MAX_RETRIES_PER_MODEL: usize = 2;

            loop {
                attempts += 1;
                match self
                    .call_generate_content(model, system_instruction, user_text)
                    .await
                {
                    Ok((res_text, prompt_tok, comp_tok)) => {
                        let dur_ms = started.elapsed().as_millis() as i64;
                        let fallback_used = idx > 0;

                        if let Some(ref db) = self.db {
                            let _ = db
                                .put_ai_cache(
                                    &cache_key,
                                    operation_type,
                                    model,
                                    &res_text,
                                    prompt_tok,
                                    comp_tok,
                                )
                                .await;
                            let _ = db
                                .record_ai_operation(
                                    None,
                                    operation_type,
                                    model,
                                    fallback_used,
                                    attempts,
                                    "SUCCESS",
                                    records_count,
                                    prompt_tok,
                                    comp_tok,
                                    dur_ms,
                                    None,
                                )
                                .await;
                        }
                        return Ok(res_text);
                    }
                    Err(err) => {
                        let msg = err.to_string();
                        let is_rate_limit = msg.contains("429") || msg.contains("RESOURCE_EXHAUSTED");
                        let is_timeout = msg.contains("timed out")
                            || msg.contains("timeout")
                            || msg.contains("conexão")
                            || msg.contains("connection");
                        let is_transient = is_rate_limit
                            || is_timeout
                            || msg.contains("503")
                            || msg.contains("404")
                            || msg.contains("NOT_FOUND")
                            || msg.contains("UNAVAILABLE");

                        if is_rate_limit && retry_count < MAX_RETRIES_PER_MODEL {
                            retry_count += 1;
                            let delay_ms = 1500 * (retry_count as u64);
                            debug!(
                                "Rate limit 429 no modelo '{}'. Aguardando {}ms para retry ({}/{})...",
                                model, delay_ms, retry_count, MAX_RETRIES_PER_MODEL
                            );
                            tokio::time::sleep(Duration::from_millis(delay_ms)).await;
                            continue;
                        }

                        if is_transient {
                            debug!(
                                "Modelo '{}' indisponível no momento ({}); tentando próximo modelo candidato...",
                                model,
                                msg.lines().next().unwrap_or("")
                            );
                            last_err = Some(err);
                            break;
                        }
                        let dur_ms = started.elapsed().as_millis() as i64;
                        if let Some(ref db) = self.db {
                            let _ = db
                                .record_ai_operation(
                                    None,
                                    operation_type,
                                    model,
                                    idx > 0,
                                    attempts,
                                    "ERROR",
                                    records_count,
                                    0,
                                    0,
                                    dur_ms,
                                    Some(&msg),
                                )
                                .await;
                        }
                        return Err(err);
                    }
                }
            }
        }

        let final_err = last_err.unwrap_or_else(|| anyhow!("Nenhum modelo Gemini disponível"));
        let dur_ms = started.elapsed().as_millis() as i64;
        if let Some(ref db) = self.db {
            let _ = db
                .record_ai_operation(
                    None,
                    operation_type,
                    candidates.last().copied().unwrap_or("unknown"),
                    true,
                    attempts,
                    "ERROR",
                    records_count,
                    0,
                    0,
                    dur_ms,
                    Some(&final_err.to_string()),
                )
                .await;
        }
        Err(final_err)
    }

    async fn call_generate_content(
        &self,
        model: &str,
        system_instruction: &str,
        user_text: &str,
    ) -> Result<(String, i64, i64)> {
        let url = format!("{}/{}:generateContent", GEMINI_BASE_URL, model);

        let payload = json!({
            "systemInstruction": {
                "parts": [{ "text": system_instruction }]
            },
            "contents": [
                {
                    "role": "user",
                    "parts": [{ "text": user_text }]
                }
            ],
            "generationConfig": {
                "temperature": 0.1,
                "maxOutputTokens": 8192,
                "responseMimeType": "application/json"
            }
        });

        debug!("POST {}", url);

        let response = self
            .http
            .post(&url)
            .header("x-goog-api-key", &self.api_key)
            .header("Content-Type", "application/json")
            .json(&payload)
            .send()
            .await
            .with_context(|| format!("Erro de conexão ao chamar Gemini ({})", model))?;

        let status = response.status();
        let body_text = response
            .text()
            .await
            .context("Falha ao ler corpo da resposta do Gemini")?;

        if !status.is_success() {
            bail!(
                "Erro da API Gemini ({}) [HTTP {}]: {}",
                model,
                status,
                body_text
            );
        }

        let json_resp: Value = serde_json::from_str(&body_text)
            .with_context(|| format!("Resposta do Gemini não é um JSON válido: {}", body_text))?;

        let prompt_tokens = json_resp["usageMetadata"]["promptTokenCount"]
            .as_i64()
            .unwrap_or_else(|| ((system_instruction.len() + user_text.len()) / 4) as i64);
        let completion_tokens = json_resp["usageMetadata"]["candidatesTokenCount"]
            .as_i64()
            .unwrap_or(0);

        let extracted_text = json_resp["candidates"][0]["content"]["parts"][0]["text"]
            .as_str()
            .ok_or_else(|| {
                anyhow!(
                    "Campo candidates[0].content.parts[0].text ausente na resposta do Gemini: {}",
                    body_text
                )
            })?;

        Ok((
            sanitize_json_response(extracted_text),
            prompt_tokens,
            completion_tokens,
        ))
    }

    /// Extrai metadados estruturados em lote (5 a 10 vagas por chamada) usando `gemini-2.5-flash-lite`
    /// (com fallback automático e cache BLAKE3 em SQLite).
    pub async fn extract_jobs_batch(
        &self,
        jobs: &[RawJobListing],
    ) -> Result<Vec<ExtractedJobMetadata>> {
        if jobs.is_empty() {
            return Ok(Vec::new());
        }

        let user_prompt = build_batch_extraction_user_prompt(jobs);
        let raw_json = self
            .call_generate_content_with_candidates(
                "EXTRACTION_BATCH",
                jobs.len(),
                BATCH_EXTRACTION_MODELS,
                BATCH_EXTRACTION_SYSTEM_PROMPT,
                &user_prompt,
            )
            .await?;

        if let Ok(parsed) = serde_json::from_str::<BatchExtractionResponse>(&raw_json) {
            return Ok(parsed.jobs);
        }

        if let Ok(direct_vec) = serde_json::from_str::<Vec<ExtractedJobMetadata>>(&raw_json) {
            return Ok(direct_vec);
        }

        bail!(
            "Não foi possível desserializar BatchExtractionResponse do JSON retornado pelo Gemini: {}",
            raw_json
        )
    }

    /// Gera o vetor de embedding (`Vec<f32>`) usando `text-embedding-004` no endpoint `:embedContent`
    /// (com fallback automático e cache em SQLite).
    pub async fn generate_embedding(&self, text: &str) -> Result<Vec<f32>> {
        let cache_key = compute_ai_cache_key("EMBEDDING", "embedContent", text);
        if let Some(ref db) = self.db {
            if let Ok(Some(cached_json)) = db.get_ai_cache(&cache_key).await {
                if let Ok(vec) = serde_json::from_str::<Vec<f32>>(&cached_json) {
                    let _ = db
                        .record_ai_operation(
                            None,
                            "EMBEDDING",
                            "cache:blake3",
                            false,
                            0,
                            "CACHE_HIT",
                            1,
                            0,
                            0,
                            0,
                            None,
                        )
                        .await;
                    return Ok(vec);
                }
            }
        }

        let started = Instant::now();
        let mut last_err = None;
        let mut attempts = 0_i64;
        for (idx, &model) in EMBEDDING_MODELS.iter().enumerate() {
            attempts += 1;
            match self.call_embed_content(model, text).await {
                Ok(vec) => {
                    let dur_ms = started.elapsed().as_millis() as i64;
                    let est_tokens = (text.len().min(8000) / 4) as i64;
                    if let Some(ref db) = self.db {
                        if let Ok(json_str) = serde_json::to_string(&vec) {
                            let _ = db
                                .put_ai_cache(&cache_key, "EMBEDDING", model, &json_str, est_tokens, 0)
                                .await;
                        }
                        let _ = db
                            .record_ai_operation(
                                None,
                                "EMBEDDING",
                                model,
                                idx > 0,
                                attempts,
                                "SUCCESS",
                                1,
                                est_tokens,
                                0,
                                dur_ms,
                                None,
                            )
                            .await;
                    }
                    return Ok(vec);
                }
                Err(err) => {
                    let msg = err.to_string();
                    if msg.contains("404") || msg.contains("503") || msg.contains("NOT_FOUND") {
                        last_err = Some(err);
                        continue;
                    }
                    let dur_ms = started.elapsed().as_millis() as i64;
                    if let Some(ref db) = self.db {
                        let _ = db
                            .record_ai_operation(
                                None,
                                "EMBEDDING",
                                model,
                                idx > 0,
                                attempts,
                                "ERROR",
                                1,
                                0,
                                0,
                                dur_ms,
                                Some(&msg),
                            )
                            .await;
                    }
                    return Err(err);
                }
            }
        }
        Err(last_err.unwrap_or_else(|| anyhow!("Nenhum modelo de embedding disponível")))
    }

    async fn call_embed_content(&self, model: &str, text: &str) -> Result<Vec<f32>> {
        let url = format!("{}/{}:embedContent", GEMINI_BASE_URL, model);

        let truncated_text: String = text.chars().take(8000).collect();
        let payload = json!({
            "model": format!("models/{}", model),
            "content": {
                "parts": [{ "text": truncated_text }]
            }
        });

        debug!("POST {}", url);

        let response = self
            .http
            .post(&url)
            .header("x-goog-api-key", &self.api_key)
            .header("Content-Type", "application/json")
            .json(&payload)
            .send()
            .await
            .context("Erro de conexão ao chamar endpoint :embedContent do Gemini")?;

        let status = response.status();
        let body_text = response
            .text()
            .await
            .context("Falha ao ler resposta do endpoint :embedContent")?;

        if !status.is_success() {
            bail!(
                "Erro da API Gemini Embeddings ({}) [HTTP {}]: {}",
                model,
                status,
                body_text
            );
        }

        let json_resp: Value = serde_json::from_str(&body_text).with_context(|| {
            format!(
                "Resposta de embedding não é um JSON válido: {}",
                body_text
            )
        })?;

        let values_arr = json_resp["embedding"]["values"]
            .as_array()
            .ok_or_else(|| {
                anyhow!(
                    "Campo embedding.values ausente na resposta do Gemini: {}",
                    body_text
                )
            })?;

        let mut embedding = Vec::with_capacity(values_arr.len());
        for val in values_arr {
            let num = val.as_f64().ok_or_else(|| {
                anyhow!("Elemento não-numérico encontrado em embedding.values")
            })?;
            embedding.push(num as f32);
        }

        Ok(embedding)
    }

    /// Executa análise profunda de compatibilidade e ATS usando `gemini-2.5-flash`
    /// (com fallback automático para `gemini-3.5-flash` / `gemini-3.6-flash`).
    pub async fn analyze_job_match(
        &self,
        resume_md: &str,
        job: &JobRecord,
    ) -> Result<MatchAnalysisResult> {
        let default_profile = CandidateProfile::default();
        self.analyze_job_match_with_profile(resume_md, job, &default_profile)
            .await
    }

    pub async fn analyze_job_match_with_profile(
        &self,
        resume_md: &str,
        job: &JobRecord,
        profile: &CandidateProfile,
    ) -> Result<MatchAnalysisResult> {
        let user_prompt = build_match_analysis_user_prompt_with_profile(resume_md, job, profile);
        let call_res = self
            .call_generate_content_with_candidates(
                "MATCH_ANALYSIS",
                1,
                MATCH_ANALYSIS_MODELS,
                MATCH_ANALYSIS_SYSTEM_PROMPT,
                &user_prompt,
            )
            .await;

        match call_res {
            Ok(raw_json) => {
                let result: MatchAnalysisResult = serde_json::from_str(&raw_json).with_context(|| {
                    format!(
                        "Falha ao desserializar MatchAnalysisResult do JSON retornado pelo Gemini: {}",
                        raw_json
                    )
                })?;
                Ok(result)
            }
            Err(err) => {
                let err_str = err.to_string();
                if err_str.contains("429")
                    || err_str.contains("RESOURCE_EXHAUSTED")
                    || err_str.contains("quota")
                    || err_str.contains("Nenhum modelo Gemini disponível")
                    || err_str.contains("timed out")
                    || err_str.contains("conexão")
                {
                    warn!(
                        "Indisponibilidade ou cota esgotada na API Gemini. Ativando análise heurística determinística de contingência para vaga #{}: {}",
                        job.id,
                        err_str.lines().next().unwrap_or("")
                    );
                    let fallback = Self::build_heuristic_fallback_match(resume_md, job, profile);
                    Ok(fallback)
                } else {
                    Err(err)
                }
            }
        }
    }

    /// Gera análise heurística determinística de contingência caso a cota do Gemini esteja esgotada.
    pub fn build_heuristic_fallback_match(
        resume_md: &str,
        job: &JobRecord,
        profile: &CandidateProfile,
    ) -> MatchAnalysisResult {
        let job_loc = job.location.as_deref().unwrap_or("");
        let job_title = job.title.as_deref().unwrap_or("");
        let is_remote = job.work_model.as_deref() == Some("Remoto")
            || job_loc.to_lowercase().contains("remoto");

        let loc_compatible = (is_remote && profile.accept_remote != 0)
            || crate::normalizer::locations_are_compatible(job_loc, &profile.base_city)
            || (!profile.accepted_region.is_empty()
                && profile
                    .accepted_region
                    .split(',')
                    .any(|r| crate::normalizer::locations_are_compatible(job_loc, r.trim())));

        let mut base_score: f32 = 45.0;
        let mut reasons = Vec::new();
        let mut matching_skills = Vec::new();
        let mut missing_skills = Vec::new();

        // 1. Avaliação de Localidade
        if loc_compatible {
            base_score += 20.0;
            reasons.push(format!("Localização ({}) compatível com seu perfil.", job_loc));
        } else if !is_remote {
            base_score -= 15.0;
            reasons.push(format!("Localização ({}) fora da sua região prioritária cadastrada.", job_loc));
        }

        // 2. Título / Palavras-chave
        let norm_title = crate::normalizer::normalize_job_title(job_title);
        let resume_lower = resume_md.to_lowercase();
        let mut match_count = 0;
        for token in norm_title.split_whitespace() {
            if token.len() > 3 {
                if resume_lower.contains(token) {
                    match_count += 1;
                    matching_skills.push(token.to_string());
                } else {
                    missing_skills.push(token.to_string());
                }
            }
        }

        if match_count >= 2 {
            base_score += 25.0;
            reasons.push("Termos principais do cargo identificados no currículo.".to_string());
        } else if match_count == 1 {
            base_score += 10.0;
            reasons.push("Correspondência parcial de palavras-chave do cargo.".to_string());
        } else {
            base_score -= 10.0;
            reasons.push("Pouca sobreposição de termos técnicos com o histórico profissional.".to_string());
        }

        // 3. Triagem determinística se presente
        if let Some(ref scr) = job.screening_verdict {
            match scr.as_str() {
                "PASS" => {
                    base_score += 10.0;
                    reasons.push("Aprovado em todos os filtros determinísticos operacionais.".to_string());
                }
                "WARN" => {
                    base_score -= 5.0;
                    reasons.push("Triagem determinística apontou ressalvas operacionais leves.".to_string());
                }
                "FAIL" => {
                    base_score -= 25.0;
                    reasons.push("Triagem determinística identificou incompatibilidade de requisitos.".to_string());
                }
                _ => {}
            }
        }

        let score = base_score.clamp(10.0, 92.0).round();
        let verdict = if score >= 70.0 {
            "apply".to_string()
        } else if score >= 40.0 {
            "maybe".to_string()
        } else {
            "skip".to_string()
        };

        MatchAnalysisResult {
            match_score: score,
            matching_skills,
            missing_skills,
            ats_suggestions: vec![
                "Análise heurística de contingência gerada localmente (cota temporária da API Gemini atingida).".to_string(),
                "Clique em 'Recalcular match' para obter a análise completa com IA assim que a cota renovar.".to_string(),
            ],
            shift_and_location_verdict: if loc_compatible {
                "Localidade compatível com perfil".to_string()
            } else {
                "Verificar distância / modalidade".to_string()
            },
            detailed_analysis_markdown: format!(
                "### Análise Heurística de Contingência (Rate Limit IA)\n\n\
                > **Nota**: Esta pontuação preliminar foi calculada localmente pelo motor de regras determinísticas porque a cota da API Gemini atingiu o limite temporário de requisições.\n\n\
                - **Pontuação Estimada**: {:.0}%\n\
                - **Veredito**: **{}**\n\
                - **Diagnóstico**: {}\n",
                score,
                verdict.to_uppercase(),
                reasons.join(" ")
            ),
            verdict: verdict.clone(),
            verdict_reasons: reasons,
            axes: FitAxes {
                skills: FitAxis {
                    level: if score >= 65.0 { "strong".to_string() } else if score >= 40.0 { "partial".to_string() } else { "weak".to_string() },
                    note: "Aferição preliminar baseada no vocabulário técnico".to_string(),
                },
                logistics: FitAxis {
                    level: if loc_compatible { "strong".to_string() } else { "weak".to_string() },
                    note: format!("Localidade: {}", job_loc),
                },
                seniority: FitAxis {
                    level: "partial".to_string(),
                    note: "Aguardando confirmação de IA".to_string(),
                },
                salary: FitAxis::default(),
                experience: FitAxis::default(),
                responsibilities: FitAxis::default(),
            },
            gaps: Vec::new(),
        }
    }

    /// Estrutura o texto bruto extraído de um currículo PDF em Markdown de alto nível e metadados.
    pub async fn structure_resume_from_text(
        &self,
        raw_text: &str,
    ) -> Result<StructuredResumeOutput> {
        let truncated = if raw_text.len() > 24_000 {
            &raw_text[..24_000]
        } else {
            raw_text
        };

        let user_prompt = format!(
            "Texto bruto extraído do PDF de currículo:\n\n```\n{}\n```\n\nEstruture o currículo em Markdown e retorne o JSON com as informações extraídas.",
            truncated
        );

        let raw_json = self
            .call_generate_content_with_candidates(
                "RESUME_STRUCTURE",
                1,
                RESUME_STRUCTURE_MODELS,
                RESUME_STRUCTURE_SYSTEM_PROMPT,
                &user_prompt,
            )
            .await?;

        let sanitized = sanitize_json_response(&raw_json);
        let result: StructuredResumeOutput = serde_json::from_str(&sanitized).with_context(|| {
            format!(
                "Falha ao desserializar StructuredResumeOutput do JSON retornado pelo Gemini: {}",
                sanitized
            )
        })?;

        Ok(result)
    }
}

pub const RESUME_STRUCTURE_SYSTEM_PROMPT: &str = r##"
Você é um especialista em estruturação de currículos técnicos e ATS (Applicant Tracking Systems).
Sua tarefa é analisar o texto bruto extraído de um arquivo PDF de currículo profissional e transformá-lo em um documento Markdown (.md) limpo, elegante e perfeitamente estruturado para leitura humana, extração de embeddings semânticos e análise de Match com vagas de emprego.

Diretrizes de Formatação Markdown:
1. Comece com '# [Nome Completo do Candidato]'
2. Adicione '## Contato & Localização' (Cidade/UF, E-mail, Telefone, LinkedIn, GitHub se identificados)
3. Adicione '## Resumo Profissional' (síntese executiva clara destacando área de atuação, senioridade e especialidades)
4. Adicione '## Competências Técnicas & Habilidades' (organizadas em tópicos ou listas: Hard skills, Soft skills, ferramentas, frameworks, bancos de dados)
5. Adicione '## Experiência Profissional' (ordenada cronologicamente: '### Cargo | Empresa (Período)', seguido de tópicos com atividades e conquistas)
6. Adicione '## Formação Acadêmica' (Curso, Instituição, Ano ou Status)
7. Adicione '## Certificações & Cursos Complementares' (se identificados)
8. Adicione '## Idiomas' (com proficiência se identificado)

Responda OBRIGATORIAMENTE em formato JSON com o seguinte schema:
{
  "markdown": "# Nome\n\n## Contato & Localização\n...",
  "suggested_name": "Nome Completo ou null",
  "suggested_location": "Cidade, UF ou null",
  "suggested_role": "Cargo Principal ou null",
  "skills": ["Habilidade 1", "Habilidade 2"]
}
"##;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StructuredResumeOutput {
    pub markdown: String,
    pub suggested_name: Option<String>,
    pub suggested_location: Option<String>,
    pub suggested_role: Option<String>,
    #[serde(default)]
    pub skills: Vec<String>,
}

pub fn compute_ai_cache_key(op_type: &str, sys_prompt: &str, user_prompt: &str) -> String {
    let payload = format!("{}||{}||{}", op_type, sys_prompt, user_prompt);
    blake3::hash(payload.as_bytes()).to_hex().to_string()
}

fn sanitize_json_response(raw: &str) -> String {
    let trimmed = raw.trim();
    if let Some(stripped) = trimmed.strip_prefix("```json") {
        if let Some(clean) = stripped.strip_suffix("```") {
            return clean.trim().to_string();
        }
    }
    if let Some(stripped) = trimmed.strip_prefix("```") {
        if let Some(clean) = stripped.strip_suffix("```") {
            return clean.trim().to_string();
        }
    }
    trimmed.to_string()
}
