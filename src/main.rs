pub mod automation;
pub mod collectors;
pub mod config;
pub mod db;
pub mod gemini;
pub mod models;
pub mod web;

use std::fs;
use std::io::{self, Stdout};
use std::time::Duration;

use anyhow::{anyhow, Context, Result};
use clap::{Parser, Subcommand};
use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use crossterm::execute;
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use ratatui::backend::CrosstermBackend;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, List, ListItem, ListState, Paragraph, Wrap};
use ratatui::{Frame, Terminal};
use tracing::{info, warn};
use tracing_subscriber::EnvFilter;

use crate::config::AppConfig;
use crate::db::Database;
use crate::gemini::GeminiClient;
use crate::models::{DbStats, JobFilter, JobRecord};

#[derive(Debug, Parser)]
#[command(
    name = "job-hunter-rs",
    version = "0.1.0",
    about = "Coleta inteligente de vagas, enriquecimento em lote via Gemini REST v1beta, busca semântica e Match Score/ATS"
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Debug, Subcommand)]
enum Commands {
    /// Coleta vagas nas fontes (Gupy + JobSpy), deduplica por hash blake3 e salva no SQLite
    Search {
        /// Termo de busca (ex: "Operador de Sistemas", "Python", "Suporte TI")
        #[arg(short, long)]
        query: String,

        /// Localização desejada (ex: "Araraquara", "São Paulo", "Brasil")
        #[arg(short = 'l', long)]
        location: Option<String>,

        /// Limite máximo de vagas por fonte
        #[arg(long, default_value_t = 20)]
        limit: usize,

        /// Filtrar apenas vagas remotas
        #[arg(short, long, default_value_t = false)]
        remote: bool,
    },

    /// Enriquecimento estruturado em lote via Gemini (gemini-2.5-flash-lite) + Embeddings (text-embedding-004)
    Enrich {
        /// Tamanho do lote enviado ao Gemini (recomendado entre 5 e 10)
        #[arg(short, long, default_value_t = 5)]
        batch_size: usize,
    },

    /// Ranking semântico em memória (similaridade de cosseno) contra o currículo local
    Rank {
        /// Caminho para o currículo em Markdown
        #[arg(short, long, default_value = "curriculo.md")]
        resume: String,

        /// Quantidade de vagas exibidas no topo do ranking
        #[arg(short, long, default_value_t = 15)]
        top: usize,
    },

    /// Análise profunda de compatibilidade (Match Score e ATS) com gemini-2.5-flash para uma vaga específica
    Match {
        /// ID da vaga na tabela SQLite (`jobs.id`)
        #[arg(short, long)]
        id: i64,

        /// Caminho para o currículo em Markdown
        #[arg(short, long, default_value = "curriculo.md")]
        resume: String,
    },

    /// Abre o painel interativo no terminal (TUI com ratatui)
    Tui,

    /// Inicia o servidor da Bancada Web Anti-Slop (Axum em localhost:8787)
    Web {
        /// Porta TCP local para servir a interface web
        #[arg(short, long, default_value_t = 8787)]
        port: u16,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    let _ = dotenvy::dotenv();

    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .with_target(false)
        .init();

    let cli = Cli::parse();
    let cfg = AppConfig::from_env();
    let db = Database::connect(&cfg.database_url).await?;

    match cli.command {
        Commands::Search {
            query,
            location,
            limit,
            remote,
        } => {
            cmd_search(&cfg, &db, &query, location.as_deref(), limit, remote).await?;
        }
        Commands::Enrich { batch_size } => {
            let count = cmd_enrich(&cfg, &db, batch_size).await?;
            println!("Enriquecimento finalizado: {} vaga(s) processada(s).", count);
        }
        Commands::Rank { resume, top } => {
            cmd_rank(&cfg, &db, &resume, top).await?;
        }
        Commands::Match { id, resume } => {
            cmd_match(&cfg, &db, id, &resume).await?;
        }
        Commands::Tui => {
            run_tui(cfg, db).await?;
        }
        Commands::Web { port } => {
            web::run_web_server(cfg, db, port).await?;
        }
    }

    Ok(())
}

async fn cmd_search(
    cfg: &AppConfig,
    db: &Database,
    query: &str,
    location: Option<&str>,
    limit: usize,
    remote: bool,
) -> Result<()> {
    println!(
        "🔍 Iniciando busca por '{}' (Local: {}, Limite: {}, Remoto: {})...",
        query,
        location.unwrap_or("Brasil"),
        limit,
        if remote { "Sim" } else { "Não" }
    );

    let report = collectors::collect_all(cfg, db, query, location, limit, remote).await?;

    println!("\n=== Resumo da Coleta ===");
    println!("  • Vagas retornadas pela Gupy   : {}", report.gupy_count);
    println!("  • Vagas retornadas pelo JobSpy : {}", report.jobspy_count);
    println!("  • Total bruto coletado         : {}", report.total_fetched);
    println!("  • Novas vagas salvas no SQLite : {}", report.newly_inserted);
    println!(
        "  • Duplicadas ignoradas (blake3): {}",
        report.duplicates_skipped
    );

    Ok(())
}

async fn cmd_enrich(cfg: &AppConfig, db: &Database, batch_size: usize) -> Result<usize> {
    let api_key = cfg.require_gemini_key()?;
    let gemini = GeminiClient::new(api_key)?;
    let effective_batch = batch_size.clamp(1, 5);

    let unenriched = db.get_unenriched_jobs(100).await?;
    if unenriched.is_empty() {
        info!("Nenhuma vaga pendente de enriquecimento (is_enriched = 0).");
        return Ok(0);
    }

    info!(
        "Encontradas {} vagas pendentes de enriquecimento. Processando em lotes de {}...",
        unenriched.len(),
        effective_batch
    );

    let mut total_enriched = 0usize;

    for chunk in unenriched.chunks(effective_batch) {
        let raw_batch: Vec<_> = chunk.iter().map(|j| j.to_raw_listing()).collect();

        let extracted_list = match gemini.extract_jobs_batch(&raw_batch).await {
            Ok(list) => list,
            Err(err) => {
                warn!("Erro na extração em lote via Gemini: {}", err);
                continue;
            }
        };

        for (idx, job_record) in chunk.iter().enumerate() {
            let expected_ext_id = job_record
                .external_id
                .clone()
                .unwrap_or_else(|| job_record.id.to_string());

            let maybe_meta = extracted_list
                .iter()
                .find(|m| m.external_id == expected_ext_id)
                .or_else(|| extracted_list.get(idx));

            let Some(raw_meta) = maybe_meta else {
                warn!(
                    "Metadados não retornados pelo modelo para a vaga ID {} ({})",
                    job_record.id,
                    job_record.title_str()
                );
                continue;
            };

            let mut meta = raw_meta.clone();
            meta.dossier
                .verify_evidence(job_record.raw_description.as_deref().unwrap_or(""));

            let embedding_text = job_record.build_embedding_text(&meta);
            let embedding_opt = match gemini.generate_embedding(&embedding_text).await {
                Ok(vec) => Some(vec),
                Err(err) => {
                    warn!(
                        "Falha ao gerar embedding para a vaga ID {}: {}",
                        job_record.id, err
                    );
                    None
                }
            };

            db.update_job_enrichment(job_record.id, &meta, embedding_opt.as_deref())
                .await?;
            total_enriched += 1;

            info!(
                "Vaga #{} enriquecida: {} [{} | {}]",
                job_record.id,
                job_record.title_str(),
                meta.work_model,
                meta.shift_schedule.as_deref().unwrap_or("N/I")
            );
        }
    }

    Ok(total_enriched)
}

async fn cmd_rank(cfg: &AppConfig, db: &Database, resume_path: &str, top: usize) -> Result<()> {
    let api_key = cfg.require_gemini_key()?;
    let gemini = GeminiClient::new(api_key)?;

    let resume_md = fs::read_to_string(resume_path)
        .with_context(|| format!("Não foi possível ler o currículo em '{}'", resume_path))?;

    println!(
        "🧠 Gerando embedding semântico para '{}' via text-embedding-004...",
        resume_path
    );
    let resume_embedding = gemini.generate_embedding(&resume_md).await?;

    let ranked = db.rank_jobs_by_embedding(&resume_embedding, top).await?;
    if ranked.is_empty() {
        println!(
            "Nenhuma vaga com embedding encontrada no banco. Execute `search` e `enrich` primeiro."
        );
        return Ok(());
    }

    println!(
        "\n=== Top {} Vagas por Similaridade Semântica (Cosseno em Memória) ===",
        ranked.len()
    );
    for (pos, (job, similarity)) in ranked.iter().enumerate() {
        let pct = (similarity * 100.0).clamp(0.0, 100.0);
        println!(
            "{:>2}. [{:>5.1}%] ID #{:<4} | {} — {} ({} | Turno: {} | Local: {})",
            pos + 1,
            pct,
            job.id,
            job.title_str(),
            job.company_str(),
            job.work_model_str(),
            job.shift_str(),
            job.location_str()
        );
        if let Some(ref url) = job.url {
            if !url.is_empty() {
                println!("    🔗 {}", url);
            }
        }
    }

    Ok(())
}

async fn cmd_match(cfg: &AppConfig, db: &Database, job_id: i64, resume_path: &str) -> Result<()> {
    let api_key = cfg.require_gemini_key()?;
    let gemini = GeminiClient::new(api_key)?;

    let job = db
        .get_job_by_id(job_id)
        .await?
        .ok_or_else(|| anyhow!("Vaga com ID #{} não encontrada no banco SQLite.", job_id))?;

    let resume_md = fs::read_to_string(resume_path)
        .with_context(|| format!("Não foi possível ler o currículo em '{}'", resume_path))?;

    println!(
        "🎯 Executando análise de Match Score & ATS (gemini-2.5-flash) para a vaga #{} ({} — {})...",
        job.id,
        job.title_str(),
        job.company_str()
    );

    let analysis = gemini.analyze_job_match(&resume_md, &job).await?;
    db.update_job_match(job.id, &analysis).await?;

    println!("\n======================================================================");
    println!(
        "RELATÓRIO DE COMPATIBILIDADE (MATCH SCORE: {:.1}/100) — VAGA #{}",
        analysis.match_score, job.id
    );
    println!(
        "Cargo   : {} | Empresa: {}",
        job.title_str(),
        job.company_str()
    );
    println!(
        "Local   : {} | Modalidade: {} | Escala/Turno: {}",
        job.location_str(),
        job.work_model_str(),
        job.shift_str()
    );
    println!("======================================================================\n");
    println!("📍 VEREDITO DE ESCALA E LOCALIZAÇÃO:\n{}\n", analysis.shift_and_location_verdict);
    println!(
        "✅ SKILLS COMPATÍVEIS:\n{}\n",
        if analysis.matching_skills.is_empty() {
            "- Nenhuma destacada".to_string()
        } else {
            analysis
                .matching_skills
                .iter()
                .map(|s| format!("  • {}", s))
                .collect::<Vec<_>>()
                .join("\n")
        }
    );
    println!(
        "⚠️ GAPS / SKILLS AUSENTES:\n{}\n",
        if analysis.missing_skills.is_empty() {
            "- Nenhum gap crítico identificado".to_string()
        } else {
            analysis
                .missing_skills
                .iter()
                .map(|s| format!("  • {}", s))
                .collect::<Vec<_>>()
                .join("\n")
        }
    );
    println!(
        "🚀 SUGESTÕES DE OTIMIZAÇÃO ATS:\n{}\n",
        if analysis.ats_suggestions.is_empty() {
            "- Sem sugestões adicionais".to_string()
        } else {
            analysis
                .ats_suggestions
                .iter()
                .map(|s| format!("  • {}", s))
                .collect::<Vec<_>>()
                .join("\n")
        }
    );
    println!("📋 ANÁLISE DETALHADA:\n{}", analysis.detailed_analysis_markdown);

    Ok(())
}

// ============================================================================
// INTERFACE DE TERMINAL INTERATIVA (TUI COM RATATUI + CROSSTERM)
// ============================================================================

const WORK_MODEL_FILTERS: &[&str] = &["Todas", "Remoto", "Hibrido", "Presencial"];
const KANBAN_STATUSES: &[&str] = &[
    "Descoberta",
    "Salva",
    "Aplicada",
    "Entrevista",
    "Descartada",
];

struct TuiApp {
    cfg: AppConfig,
    db: Database,
    stats: DbStats,
    jobs: Vec<JobRecord>,
    list_state: ListState,
    filter_idx: usize,
    detail_scroll: u16,
    status_message: String,
}

impl TuiApp {
    async fn new(cfg: AppConfig, db: Database) -> Result<Self> {
        let mut app = Self {
            cfg,
            db,
            stats: DbStats::default(),
            jobs: Vec::new(),
            list_state: ListState::default(),
            filter_idx: 0,
            detail_scroll: 0,
            status_message: "Pronto. Use ↑/↓ ou j/k para navegar, [f] filtrar, [s] status, [m] match IA, [e] enriquecer, [q] sair.".to_string(),
        };
        app.reload_data().await?;
        Ok(app)
    }

    fn current_work_model_filter(&self) -> &str {
        WORK_MODEL_FILTERS[self.filter_idx % WORK_MODEL_FILTERS.len()]
    }

    async fn reload_data(&mut self) -> Result<()> {
        self.stats = self.db.get_stats().await?;
        let wm = self.current_work_model_filter();
        let filter = JobFilter {
            work_model: if wm == "Todas" {
                None
            } else {
                Some(wm.to_string())
            },
            ..Default::default()
        };
        self.jobs = self.db.list_jobs(&filter).await?;

        if self.jobs.is_empty() {
            self.list_state.select(None);
        } else {
            let sel = self
                .list_state
                .selected()
                .unwrap_or(0)
                .min(self.jobs.len().saturating_sub(1));
            self.list_state.select(Some(sel));
        }

        Ok(())
    }

    fn selected_job(&self) -> Option<&JobRecord> {
        let idx = self.list_state.selected()?;
        self.jobs.get(idx)
    }

    fn move_selection(&mut self, delta: isize) {
        if self.jobs.is_empty() {
            self.list_state.select(None);
            return;
        }
        let current = self.list_state.selected().unwrap_or(0) as isize;
        let next = (current + delta).clamp(0, (self.jobs.len() - 1) as isize) as usize;
        self.list_state.select(Some(next));
        self.detail_scroll = 0;
    }

    async fn cycle_filter(&mut self) -> Result<()> {
        self.filter_idx = (self.filter_idx + 1) % WORK_MODEL_FILTERS.len();
        self.detail_scroll = 0;
        self.reload_data().await?;
        self.status_message = format!(
            "Filtro de modalidade alterado para: {}",
            self.current_work_model_filter()
        );
        Ok(())
    }

    async fn cycle_kanban_status(&mut self) -> Result<()> {
        let Some(job) = self.selected_job().cloned() else {
            self.status_message = "Nenhuma vaga selecionada.".to_string();
            return Ok(());
        };

        let current_status = job.status_str();
        let current_pos = KANBAN_STATUSES
            .iter()
            .position(|s| s.eq_ignore_ascii_case(current_status))
            .unwrap_or(0);
        let next_status = KANBAN_STATUSES[(current_pos + 1) % KANBAN_STATUSES.len()];

        self.db.update_job_status(job.id, next_status).await?;
        self.reload_data().await?;
        self.status_message = format!(
            "Status da vaga #{} atualizado para '{}'.",
            job.id, next_status
        );
        Ok(())
    }

    async fn trigger_match_for_selected(&mut self) -> Result<()> {
        let Some(job) = self.selected_job().cloned() else {
            self.status_message = "Nenhuma vaga selecionada para cálculo de Match Score.".to_string();
            return Ok(());
        };

        let api_key = match self.cfg.require_gemini_key() {
            Ok(k) => k.to_string(),
            Err(err) => {
                self.status_message = format!("Erro: {}", err);
                return Ok(());
            }
        };

        let resume_md = match fs::read_to_string(&self.cfg.resume_path) {
            Ok(content) => content,
            Err(err) => {
                self.status_message = format!(
                    "Erro ao ler '{}': {}",
                    self.cfg.resume_path, err
                );
                return Ok(());
            }
        };

        let gemini = GeminiClient::new(api_key)?;
        match gemini.analyze_job_match(&resume_md, &job).await {
            Ok(analysis) => {
                self.db.update_job_match(job.id, &analysis).await?;
                self.reload_data().await?;
                self.status_message = format!(
                    "Match Score calculado para vaga #{}: {:.1}/100",
                    job.id, analysis.match_score
                );
            }
            Err(err) => {
                self.status_message = format!("Falha no Gemini Match: {}", err);
            }
        }

        Ok(())
    }

    async fn trigger_enrich_pending(&mut self) -> Result<()> {
        match cmd_enrich(&self.cfg, &self.db, 5).await {
            Ok(count) => {
                self.reload_data().await?;
                self.status_message =
                    format!("Enriquecimento concluído: {} vaga(s) atualizada(s).", count);
            }
            Err(err) => {
                self.status_message = format!("Falha ao enriquecer vagas: {}", err);
            }
        }
        Ok(())
    }
}

async fn run_tui(cfg: AppConfig, db: Database) -> Result<()> {
    enable_raw_mode().context("Falha ao ativar raw mode no terminal")?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)
        .context("Falha ao entrar na Alternate Screen do terminal")?;

    let backend = CrosstermBackend::new(stdout);
    let mut terminal =
        Terminal::new(backend).context("Falha ao inicializar terminal Ratatui")?;

    let app_result = run_tui_loop(&mut terminal, cfg, db).await;

    let _ = disable_raw_mode();
    let _ = execute!(terminal.backend_mut(), LeaveAlternateScreen);
    let _ = terminal.show_cursor();

    app_result
}

async fn run_tui_loop(
    terminal: &mut Terminal<CrosstermBackend<Stdout>>,
    cfg: AppConfig,
    db: Database,
) -> Result<()> {
    let mut app = TuiApp::new(cfg, db).await?;

    loop {
        terminal.draw(|frame| draw_tui_frame(frame, &mut app))?;

        if event::poll(Duration::from_millis(200))? {
            if let Event::Key(key) = event::read()? {
                if key.kind != KeyEventKind::Press {
                    continue;
                }

                match key.code {
                    KeyCode::Char('q') | KeyCode::Esc => break,
                    KeyCode::Down | KeyCode::Char('j') => app.move_selection(1),
                    KeyCode::Up | KeyCode::Char('k') => app.move_selection(-1),
                    KeyCode::PageDown | KeyCode::Char('d') => {
                        app.detail_scroll = app.detail_scroll.saturating_add(5);
                    }
                    KeyCode::PageUp | KeyCode::Char('u') => {
                        app.detail_scroll = app.detail_scroll.saturating_sub(5);
                    }
                    KeyCode::Char('f') => {
                        app.cycle_filter().await?;
                    }
                    KeyCode::Char('s') => {
                        app.cycle_kanban_status().await?;
                    }
                    KeyCode::Char('m') => {
                        app.status_message =
                            "Calculando Match Score via Gemini (gemini-2.5-flash)... aguarde."
                                .to_string();
                        terminal.draw(|frame| draw_tui_frame(frame, &mut app))?;
                        app.trigger_match_for_selected().await?;
                    }
                    KeyCode::Char('e') => {
                        app.status_message =
                            "Enriquecendo vagas pendentes em lote via Gemini... aguarde."
                                .to_string();
                        terminal.draw(|frame| draw_tui_frame(frame, &mut app))?;
                        app.trigger_enrich_pending().await?;
                    }
                    _ => {}
                }
            }
        }
    }

    Ok(())
}

fn draw_tui_frame(frame: &mut Frame, app: &mut TuiApp) {
    let vertical_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(10),
            Constraint::Length(3),
        ])
        .split(frame.area());

    draw_top_bar(frame, app, vertical_chunks[0]);

    let main_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(42), Constraint::Percentage(58)])
        .split(vertical_chunks[1]);

    draw_jobs_list_panel(frame, app, main_chunks[0]);
    draw_job_detail_panel(frame, app, main_chunks[1]);
    draw_bottom_bar(frame, app, vertical_chunks[2]);
}

fn draw_top_bar(frame: &mut Frame, app: &TuiApp, area: Rect) {
    let header_line = Line::from(vec![
        Span::styled(
            " JOB HUNTER RS ",
            Style::default()
                .fg(Color::Black)
                .bg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw("  Total: "),
        Span::styled(
            app.stats.total_jobs.to_string(),
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(" | Enriquecidas: "),
        Span::styled(
            app.stats.enriched_jobs.to_string(),
            Style::default()
                .fg(Color::Green)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(" | Salvas/Aplicadas: "),
        Span::styled(
            app.stats.saved_or_applied_jobs.to_string(),
            Style::default()
                .fg(Color::Magenta)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(" | Filtro Modalidade: "),
        Span::styled(
            format!("[{}]", app.current_work_model_filter()),
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
    ]);

    let block = Block::default()
        .borders(Borders::ALL)
        .title(" Estatísticas do Banco & Filtros Ativos ");
    let paragraph = Paragraph::new(header_line).block(block);
    frame.render_widget(paragraph, area);
}

fn draw_jobs_list_panel(frame: &mut Frame, app: &mut TuiApp, area: Rect) {
    let items: Vec<ListItem> = if app.jobs.is_empty() {
        vec![ListItem::new(Line::from(Span::styled(
            "Nenhuma vaga para o filtro atual. Use `search` na CLI ou pressione [f].",
            Style::default().fg(Color::DarkGray),
        )))]
    } else {
        app.jobs
            .iter()
            .map(|job| {
                let score_str = match job.match_score {
                    Some(s) => format!("[{:>3.0}%]", s),
                    None => "[ -- ]".to_string(),
                };
                let score_color = match job.match_score {
                    Some(s) if s >= 75.0 => Color::Green,
                    Some(s) if s >= 50.0 => Color::Yellow,
                    Some(_) => Color::Red,
                    None => Color::DarkGray,
                };

                let line = Line::from(vec![
                    Span::styled(
                        score_str,
                        Style::default()
                            .fg(score_color)
                            .add_modifier(Modifier::BOLD),
                    ),
                    Span::raw(" "),
                    Span::styled(
                        job.title_str().to_string(),
                        Style::default().fg(Color::White).add_modifier(Modifier::BOLD),
                    ),
                    Span::raw(" - "),
                    Span::styled(
                        job.company_str().to_string(),
                        Style::default().fg(Color::Cyan),
                    ),
                    Span::raw(format!(
                        " ({} | {})",
                        job.work_model_str(),
                        job.shift_str()
                    )),
                ]);
                ListItem::new(line)
            })
            .collect()
    };

    let list = List::new(items)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(format!(" Vagas ({}) ", app.jobs.len())),
        )
        .highlight_style(
            Style::default()
                .bg(Color::DarkGray)
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol("▶ ");

    frame.render_stateful_widget(list, area, &mut app.list_state);
}

fn draw_job_detail_panel(frame: &mut Frame, app: &TuiApp, area: Rect) {
    let block = Block::default()
        .borders(Borders::ALL)
        .title(" Detalhes da Vaga (PgUp/PgDn ou u/d para rolar) ");

    let Some(job) = app.selected_job() else {
        let empty = Paragraph::new("Selecione uma vaga na lista à esquerda para visualizar seus detalhes completos.")
            .block(block)
            .wrap(Wrap { trim: false });
        frame.render_widget(empty, area);
        return;
    };

    let skills = job.required_skills();
    let skills_str = if skills.is_empty() {
        "Não informadas / Vaga ainda não enriquecida".to_string()
    } else {
        skills.join(", ")
    };

    let red_flags = job.red_flags();
    let red_flags_str = if red_flags.is_empty() {
        "Nenhum alerta crítico detectado".to_string()
    } else {
        red_flags.join(" | ")
    };

    let match_score_str = job
        .match_score
        .map(|s| format!("{:.1}/100", s))
        .unwrap_or_else(|| "Não calculado (pressione [m])".to_string());

    let mut lines: Vec<Line> = vec![
        Line::from(vec![
            Span::styled("ID: ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                format!("#{} ({})", job.id, job.source.as_deref().unwrap_or("n/d")),
                Style::default().fg(Color::White).add_modifier(Modifier::BOLD),
            ),
            Span::raw("   "),
            Span::styled("Status Kanban: ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                format!("[{}]", job.status_str()),
                Style::default()
                    .fg(Color::Magenta)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw("   "),
            Span::styled("Match Score: ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                match_score_str,
                Style::default()
                    .fg(Color::Green)
                    .add_modifier(Modifier::BOLD),
            ),
        ]),
        Line::from(vec![
            Span::styled("Título: ", Style::default().fg(Color::Yellow)),
            Span::styled(
                job.title_str().to_string(),
                Style::default().fg(Color::White).add_modifier(Modifier::BOLD),
            ),
        ]),
        Line::from(vec![
            Span::styled("Empresa: ", Style::default().fg(Color::Yellow)),
            Span::raw(job.company_str().to_string()),
            Span::raw(" | "),
            Span::styled("Local: ", Style::default().fg(Color::Yellow)),
            Span::raw(job.location_str().to_string()),
        ]),
        Line::from(vec![
            Span::styled("Modalidade: ", Style::default().fg(Color::Cyan)),
            Span::raw(job.work_model_str().to_string()),
            Span::raw(" | "),
            Span::styled("Contrato: ", Style::default().fg(Color::Cyan)),
            Span::raw(job.contract_type.as_deref().unwrap_or("Nao Informado")),
            Span::raw(" | "),
            Span::styled("Senioridade: ", Style::default().fg(Color::Cyan)),
            Span::raw(job.seniority.as_deref().unwrap_or("Nao Informado")),
        ]),
        Line::from(vec![
            Span::styled("Escala / Turno: ", Style::default().fg(Color::Green)),
            Span::styled(
                job.shift_str().to_string(),
                Style::default().fg(Color::White).add_modifier(Modifier::BOLD),
            ),
            Span::raw(" | "),
            Span::styled("Salário Estimado: ", Style::default().fg(Color::Green)),
            Span::raw(job.salary_estimate_brl.as_deref().unwrap_or("Não informado")),
        ]),
        Line::from(vec![
            Span::styled("URL: ", Style::default().fg(Color::DarkGray)),
            Span::raw(job.url.as_deref().unwrap_or("N/D")),
        ]),
        Line::from(""),
        Line::from(Span::styled(
            "── Resumo da IA ──────────────────────────────────────────",
            Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
        )),
        Line::from(
            job.ai_summary
                .as_deref()
                .unwrap_or("Vaga ainda não enriquecida pela IA. Pressione [e] para enriquecer."),
        ),
        Line::from(""),
        Line::from(Span::styled(
            "── Skills Obrigatórias ───────────────────────────────────",
            Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD),
        )),
        Line::from(skills_str),
        Line::from(""),
        Line::from(Span::styled(
            "── Red Flags / Alertas ───────────────────────────────────",
            Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
        )),
        Line::from(red_flags_str),
        Line::from(""),
        Line::from(Span::styled(
            "── Análise de Match & ATS ────────────────────────────────",
            Style::default().fg(Color::Green).add_modifier(Modifier::BOLD),
        )),
    ];

    if let Some(ref analysis_md) = job.match_analysis_md {
        for md_line in analysis_md.lines() {
            lines.push(Line::from(md_line.to_string()));
        }
    } else {
        lines.push(Line::from(
            "Nenhuma análise de compatibilidade registrada. Pressione [m] para analisar contra o curriculo.md.",
        ));
    }

    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        "── Descrição Bruta ───────────────────────────────────────",
        Style::default()
            .fg(Color::DarkGray)
            .add_modifier(Modifier::BOLD),
    )));
    for raw_line in job.raw_description.as_deref().unwrap_or("").lines().take(80) {
        lines.push(Line::from(raw_line.to_string()));
    }

    let paragraph = Paragraph::new(lines)
        .block(block)
        .scroll((app.detail_scroll, 0))
        .wrap(Wrap { trim: false });

    frame.render_widget(paragraph, area);
}

fn draw_bottom_bar(frame: &mut Frame, app: &TuiApp, area: Rect) {
    let footer_line = Line::from(vec![
        Span::styled(
            " [↑/↓ j/k] ",
            Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
        ),
        Span::raw("Navegar "),
        Span::styled(
            " [f] ",
            Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD),
        ),
        Span::raw("Filtro Modalidade "),
        Span::styled(
            " [s] ",
            Style::default().fg(Color::Magenta).add_modifier(Modifier::BOLD),
        ),
        Span::raw("Mudar Status "),
        Span::styled(
            " [m] ",
            Style::default().fg(Color::Green).add_modifier(Modifier::BOLD),
        ),
        Span::raw("Match Score IA "),
        Span::styled(
            " [e] ",
            Style::default().fg(Color::Blue).add_modifier(Modifier::BOLD),
        ),
        Span::raw("Enriquecer "),
        Span::styled(
            " [q] ",
            Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
        ),
        Span::raw("Sair  │ "),
        Span::styled(&app.status_message, Style::default().fg(Color::Gray)),
    ]);

    let block = Block::default().borders(Borders::ALL).title(" Atalhos & Status ");
    let paragraph = Paragraph::new(footer_line).block(block);
    frame.render_widget(paragraph, area);
}
