use std::time::Duration;

use anyhow::{anyhow, bail, Context, Result};
use reqwest::Client;
use serde_json::{json, Value};
use tracing::debug;

use crate::gemini::prompts::{
    build_batch_extraction_user_prompt, build_match_analysis_user_prompt,
    BATCH_EXTRACTION_SYSTEM_PROMPT, MATCH_ANALYSIS_SYSTEM_PROMPT,
};
use crate::models::{
    BatchExtractionResponse, ExtractedJobMetadata, JobRecord, MatchAnalysisResult, RawJobListing,
};

const GEMINI_BASE_URL: &str = "https://generativelanguage.googleapis.com/v1beta/models";
const BATCH_EXTRACTION_MODELS: &[&str] = &[
    "gemini-2.5-flash-lite",
    "gemini-3.5-flash-lite",
    "gemini-3.1-flash-lite",
    "gemini-flash-lite-latest",
];
const MATCH_ANALYSIS_MODELS: &[&str] = &[
    "gemini-2.5-flash",
    "gemini-3.5-flash",
    "gemini-3.6-flash",
    "gemini-3.5-flash-lite",
];
const EMBEDDING_MODELS: &[&str] = &[
    "text-embedding-004",
    "gemini-embedding-001",
    "gemini-embedding-2",
];

#[derive(Debug, Clone)]
pub struct GeminiClient {
    http: Client,
    api_key: String,
}

impl GeminiClient {
    pub fn new(api_key: impl Into<String>) -> Result<Self> {
        let http = Client::builder()
            .timeout(Duration::from_secs(90))
            .build()
            .context("Falha ao inicializar cliente HTTP reqwest para o Gemini")?;

        Ok(Self {
            http,
            api_key: api_key.into(),
        })
    }

    async fn call_generate_content_with_candidates(
        &self,
        candidates: &[&str],
        system_instruction: &str,
        user_text: &str,
    ) -> Result<String> {
        let mut last_err = None;

        for &model in candidates {
            match self
                .call_generate_content(model, system_instruction, user_text)
                .await
            {
                Ok(res) => return Ok(res),
                Err(err) => {
                    let msg = err.to_string();
                    if msg.contains("404")
                        || msg.contains("503")
                        || msg.contains("429")
                        || msg.contains("403")
                        || msg.contains("NOT_FOUND")
                        || msg.contains("UNAVAILABLE")
                    {
                        debug!(
                            "Modelo '{}' indisponível no momento ({}); tentando próximo modelo candidato...",
                            model,
                            msg.lines().next().unwrap_or("")
                        );
                        last_err = Some(err);
                        continue;
                    }
                    return Err(err);
                }
            }
        }

        Err(last_err.unwrap_or_else(|| anyhow!("Nenhum modelo Gemini disponível")))
    }

    async fn call_generate_content(
        &self,
        model: &str,
        system_instruction: &str,
        user_text: &str,
    ) -> Result<String> {
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

        let extracted_text = json_resp["candidates"][0]["content"]["parts"][0]["text"]
            .as_str()
            .ok_or_else(|| {
                anyhow!(
                    "Campo candidates[0].content.parts[0].text ausente na resposta do Gemini: {}",
                    body_text
                )
            })?;

        Ok(sanitize_json_response(extracted_text))
    }

    /// Extrai metadados estruturados em lote (5 a 10 vagas por chamada) usando `gemini-2.5-flash-lite`
    /// (com fallback automático caso o modelo esteja indisponível na chave).
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
    /// (com fallback automático para `gemini-embedding-001` caso `text-embedding-004` retorne 404).
    pub async fn generate_embedding(&self, text: &str) -> Result<Vec<f32>> {
        let mut last_err = None;
        for &model in EMBEDDING_MODELS {
            match self.call_embed_content(model, text).await {
                Ok(vec) => return Ok(vec),
                Err(err) => {
                    let msg = err.to_string();
                    if msg.contains("404") || msg.contains("503") || msg.contains("NOT_FOUND") {
                        last_err = Some(err);
                        continue;
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
        let user_prompt = build_match_analysis_user_prompt(resume_md, job);
        let raw_json = self
            .call_generate_content_with_candidates(
                MATCH_ANALYSIS_MODELS,
                MATCH_ANALYSIS_SYSTEM_PROMPT,
                &user_prompt,
            )
            .await?;

        let result: MatchAnalysisResult = serde_json::from_str(&raw_json).with_context(|| {
            format!(
                "Falha ao desserializar MatchAnalysisResult do JSON retornado pelo Gemini: {}",
                raw_json
            )
        })?;

        Ok(result)
    }
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
