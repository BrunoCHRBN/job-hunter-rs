use std::path::Path;
use tokio::process::Command;
use tracing::{debug, info, warn};

use crate::models::RawJobListing;

pub async fn fetch_jobspy_jobs(
    python_bin: &str,
    query: &str,
    location: &str,
    results: usize,
    hours_old: u32,
    remote: bool,
) -> Vec<RawJobListing> {
    let script_path = "scripts/jobspy_bridge.py";
    if !Path::new(script_path).exists() {
        warn!(
            "Script '{}' não encontrado no diretório de trabalho atual. Pulando coletor JobSpy.",
            script_path
        );
        return Vec::new();
    }

    let mut cmd = Command::new(python_bin);
    cmd.arg(script_path)
        .arg("--query")
        .arg(query)
        .arg("--location")
        .arg(location)
        .arg("--results")
        .arg(results.to_string())
        .arg("--hours-old")
        .arg(hours_old.to_string());

    if remote {
        cmd.arg("--remote");
    }

    debug!(
        "Executando bridge JobSpy com '{} {} --query {} --location {}'",
        python_bin, script_path, query, location
    );

    let output = match cmd.output().await {
        Ok(out) => out,
        Err(err) => {
            warn!(
                "Falha ao invocar binário Python '{}' para JobSpy: {}. Retornando lista vazia.",
                python_bin, err
            );
            return Vec::new();
        }
    };

    let stderr_text = String::from_utf8_lossy(&output.stderr);
    if !stderr_text.trim().is_empty() {
        warn!("Aviso do bridge JobSpy (stderr): {}", stderr_text.trim());
    }

    if !output.status.success() {
        warn!(
            "Bridge JobSpy encerrou com código de saída {:?}. Retornando lista vazia.",
            output.status.code()
        );
        return Vec::new();
    }

    let stdout_text = String::from_utf8_lossy(&output.stdout);
    let trimmed = stdout_text.trim();
    if trimmed.is_empty() {
        warn!("Bridge JobSpy produziu saída vazia no stdout.");
        return Vec::new();
    }

    match serde_json::from_str::<Vec<RawJobListing>>(trimmed) {
        Ok(jobs) => {
            info!("Coletor JobSpy retornou {} vagas para '{}'", jobs.len(), query);
            jobs
        }
        Err(err) => {
            warn!(
                "Falha ao desserializar JSON retornado pelo JobSpy: {}. Saída bruta: {}",
                err,
                trimmed.chars().take(240).collect::<String>()
            );
            Vec::new()
        }
    }
}
