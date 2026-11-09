use std::env;
use std::path::Path;
use anyhow::{bail, Result};

#[derive(Debug, Clone)]
pub struct AppConfig {
    pub gemini_api_key: String,
    pub database_url: String,
    pub python_bin: String,
    pub resume_path: String,
}

impl AppConfig {
    pub fn from_env() -> Self {
        let _ = dotenvy::dotenv();

        let gemini_api_key = env::var("GEMINI_API_KEY").unwrap_or_default();
        let database_url = env::var("DATABASE_URL")
            .unwrap_or_else(|_| "sqlite://jobs.db".to_string());
        let python_bin = match env::var("PYTHON_BIN") {
            Ok(val) if !val.trim().is_empty() && val != "python3" => val,
            _ => {
                if Path::new(".venv/bin/python").exists() {
                    ".venv/bin/python".to_string()
                } else {
                    "python3".to_string()
                }
            }
        };
        let resume_path = env::var("RESUME_PATH")
            .unwrap_or_else(|_| "curriculo.md".to_string());

        Self {
            gemini_api_key,
            database_url,
            python_bin,
            resume_path,
        }
    }

    pub fn require_gemini_key(&self) -> Result<&str> {
        let trimmed = self.gemini_api_key.trim();
        if trimmed.is_empty() || trimmed == "SUA_CHAVE_GEMINI_AQUI" {
            bail!(
                "Variável GEMINI_API_KEY não configurada ou inválida. Defina sua chave do Google AI Studio no arquivo .env ou no ambiente."
            );
        }
        Ok(trimmed)
    }
}
