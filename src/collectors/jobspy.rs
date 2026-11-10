use std::path::PathBuf;
use tokio::process::Command;
use tracing::{debug, info, warn};

use crate::execution::ExecutionHandle;
use crate::models::RawJobListing;

#[derive(Debug, Clone, Default)]
pub struct JobSpyFetchResult {
    pub jobs: Vec<RawJobListing>,
    pub linkedin_count: usize,
    pub indeed_count: usize,
    pub glassdoor_count: usize,
    pub warnings: Vec<String>,
    pub error: Option<String>,
}

fn resolve_script_path() -> Option<PathBuf> {
    let candidates = [
        PathBuf::from("scripts/jobspy_bridge.py"),
        PathBuf::from("job-hunter-rs/scripts/jobspy_bridge.py"),
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("scripts/jobspy_bridge.py"),
    ];
    candidates.into_iter().find(|path| path.exists())
}

pub async fn fetch_jobspy_jobs(
    python_bin: &str,
    query: &str,
    location: &str,
    results: usize,
    hours_old: u32,
    remote: bool,
    exec_handle: Option<&ExecutionHandle>,
) -> JobSpyFetchResult {
    let mut res = JobSpyFetchResult::default();

    let script_path = match resolve_script_path() {
        Some(p) => p,
        None => {
            let msg = "Script 'scripts/jobspy_bridge.py' não encontrado em nenhum caminho padrão. Pulando coletor JobSpy.";
            warn!("{}", msg);
            if let Some(h) = exec_handle {
                h.emit_warn("jobspy_collector", Some("jobspy"), msg).await;
            }
            res.error = Some(msg.to_string());
            return res;
        }
    };

    if let Some(h) = exec_handle {
        h.emit_info(
            "jobspy_collector",
            Some("jobspy"),
            &format!(
                "Iniciando coleta JobSpy (LinkedIn, Indeed, Glassdoor) para '{}' em '{}' (limite: {})",
                query, location, results
            ),
        )
        .await;
    }

    let mut cmd = Command::new(python_bin);
    cmd.arg(&script_path)
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
        python_bin,
        script_path.display(),
        query,
        location
    );

    let output = match cmd.output().await {
        Ok(out) => out,
        Err(err) => {
            let msg = format!(
                "Falha ao invocar interpretador Python '{}' para JobSpy: {}. Verifique se o ambiente virtual (.venv) está configurado.",
                python_bin, err
            );
            warn!("{}", msg);
            if let Some(h) = exec_handle {
                h.emit_error("jobspy_collector", Some("jobspy"), &msg, None).await;
            }
            res.error = Some(msg);
            return res;
        }
    };

    let stderr_text = String::from_utf8_lossy(&output.stderr);
    for line in stderr_text.lines() {
        let trimmed_line = line.trim();
        if trimmed_line.is_empty() {
            continue;
        }

        if trimmed_line.starts_with("JOB_SPY_ERROR:") {
            let err_msg = trimmed_line.trim_start_matches("JOB_SPY_ERROR:").trim();
            warn!("JobSpy Dependência: {}", err_msg);
            if let Some(h) = exec_handle {
                h.emit_error("jobspy_collector", Some("jobspy"), err_msg, None).await;
            }
            res.error = Some(err_msg.to_string());
        } else if trimmed_line.starts_with("SITE_ERR:") {
            let warn_msg = trimmed_line.trim_start_matches("SITE_ERR:").trim();
            res.warnings.push(warn_msg.to_string());
            if let Some(h) = exec_handle {
                h.emit_warn(
                    "jobspy_collector",
                    Some("jobspy"),
                    &format!("Alerta ao consultar fonte: {}", warn_msg),
                )
                .await;
            }
        } else if trimmed_line.starts_with("SITE_STATS:") {
            let stats_part = trimmed_line.trim_start_matches("SITE_STATS:").trim();
            if let Some((site, count_str)) = stats_part.split_once('=') {
                let cnt = count_str.parse::<usize>().unwrap_or(0);
                match site {
                    "linkedin" => res.linkedin_count = cnt,
                    "indeed" => res.indeed_count = cnt,
                    "glassdoor" => res.glassdoor_count = cnt,
                    _ => {}
                }
            }
        } else {
            debug!("Aviso JobSpy (stderr): {}", trimmed_line);
        }
    }

    if !output.status.success() && res.error.is_none() {
        let msg = format!(
            "Bridge JobSpy encerrou com código de saída {:?}. Detalhe no log de erro.",
            output.status.code()
        );
        warn!("{}", msg);
        if let Some(h) = exec_handle {
            h.emit_error("jobspy_collector", Some("jobspy"), &msg, Some(&stderr_text)).await;
        }
        res.error = Some(msg);
        return res;
    }

    let stdout_text = String::from_utf8_lossy(&output.stdout);
    let trimmed = stdout_text.trim();
    if trimmed.is_empty() {
        warn!("Bridge JobSpy produziu saída vazia no stdout.");
        if let Some(h) = exec_handle {
            h.emit_warn(
                "jobspy_collector",
                Some("jobspy"),
                "Nenhuma saída JSON produzida pelo bridge JobSpy.",
            )
            .await;
        }
        return res;
    }

    match serde_json::from_str::<Vec<RawJobListing>>(trimmed) {
        Ok(jobs) => {
            let total = jobs.len();
            info!("Coletor JobSpy retornou {} vagas para '{}'", total, query);
            if let Some(h) = exec_handle {
                h.emit_info(
                    "jobspy_collector",
                    Some("jobspy"),
                    &format!(
                        "JobSpy retornou {} vagas no total (LinkedIn: {}, Indeed: {}, Glassdoor: {})",
                        total, res.linkedin_count, res.indeed_count, res.glassdoor_count
                    ),
                )
                .await;
                h.record_metric("jobspy_total_fetched", total as f64, "count").await.ok();
                h.record_metric("jobspy_linkedin_fetched", res.linkedin_count as f64, "count").await.ok();
                h.record_metric("jobspy_indeed_fetched", res.indeed_count as f64, "count").await.ok();
                h.record_metric("jobspy_glassdoor_fetched", res.glassdoor_count as f64, "count").await.ok();
            }
            res.jobs = jobs;
            res
        }
        Err(err) => {
            let msg = format!(
                "Falha ao desserializar JSON retornado pelo JobSpy: {}. Saída bruta: {}",
                err,
                trimmed.chars().take(240).collect::<String>()
            );
            warn!("{}", msg);
            if let Some(h) = exec_handle {
                h.emit_error("jobspy_collector", Some("jobspy"), &msg, Some(trimmed)).await;
            }
            res.error = Some(msg);
            res
        }
    }
}
