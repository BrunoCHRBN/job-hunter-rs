# Job Hunter RS

Aplicação de linha de comando (**CLI**), interface de terminal interativa (**TUI** com `ratatui`) e bancada web (**Web UI** com `axum` e sistema de design **Anti-AI-Slop**) construída em **Rust (Edition 2021)** para coleta automatizada de vagas de emprego, deduplicação determinística por hash `blake3`, enriquecimento estruturado em lote via IA, busca semântica em memória e análise profunda de compatibilidade curricular (**Match Score & ATS**) via API REST `v1beta` do Google AI Studio.

---

## Sumário

1. [Visão Geral e Arquitetura](#1-visão-geral-e-arquitetura)
2. [Design da Interface Web (Políticas e Diretrizes Anti-AI-Slop)](#2-design-da-interface-web-políticas-e-diretrizes-anti-ai-slop)
3. [Design da Interface de Terminal (TUI com `ratatui`)](#3-design-da-interface-de-terminal-tui-com-ratatui)
4. [Pipeline de Inteligência Artificial (Google AI Studio REST `v1beta`)](#4-pipeline-de-inteligência-artificial-google-ai-studio-rest-v1beta)
5. [Deduplicação `blake3` e Busca Semântica em Memória](#5-deduplicação-blake3-e-busca-semântica-em-memória)
6. [Instalação, Configuração e Uso](#6-instalação-configuração-e-uso)

---

## 1. Visão Geral e Arquitetura

O **Job Hunter RS** opera a partir de um único executável em Rust que compartilha o mesmo banco de dados relacional SQLite (`jobs.db`) e o mesmo currículo local em Markdown (`curriculo.md`) entre três modos de operação: **CLI**, **TUI** e **Web UI**.

```text
.
├── Cargo.toml                 # Manifesto Rust (Edition 2021) e dependências
├── GEMINI.md                  # Especificação técnica e contratos REST v1beta
├── README.md                  # Documentação geral, arquitetura e guia de design UI/TUI
├── .env.example               # Template de variáveis de ambiente
├── curriculo.md               # Currículo base em Markdown utilizado no Ranking e ATS
├── migrations/
│   ├── 001_init.sql           # Schema SQLite inicial (tabela `jobs` e índices)
│   ├── 002_dossie.sql         # Dossiê v2 (`dossier_json` e `fit_json`)
│   └── 003_automation.sql     # Automação Zero-Touch (`candidate_profile` e `saved_searches`)
├── scripts/
│   └── jobspy_bridge.py       # Bridge Python (LinkedIn, Indeed, Glassdoor via python-jobspy)
├── requirements.txt           # Dependências Python (.venv com python-jobspy)
├── static/
│   ├── styles.css             # Estrutura base e tokens CSS (Tema Claro/Escuro)
│   ├── identity.css           # Identidade visual v2 (Ciano funcional, vereditos e responsividade)
│   ├── icon.svg               # Ícone vetorial oficial do aplicativo Desktop
│   ├── index.html             # App Shell de 4 visões (Painel, Vagas, Funil Kanban e Currículo/Perfil)
│   └── app.js                 # Lógica de Dossiê v2, Automação, Drag-and-Drop e atalhos
└── src/
    ├── main.rs                # Entrypoint (abre Web + Navegador por padrão) + CLI + TUI (ratatui)
    ├── automation.rs          # Worker Autônomo Zero-Touch (Rondas + Enriquecimento + Auto-Match)
    ├── web.rs                 # Servidor HTTP embutido (axum) e rotas REST JSON (/api/*)
    ├── config.rs              # Configuração (.env) e detecção automática de `.venv/bin/python`
    ├── models.rs              # Dossiê v2 (`Dossier`, `Provenance`, `FitAxes`, `verify_evidence`)
    ├── db.rs                  # Persistência SQLite (sqlx), hash blake3 e cosine_similarity
    ├── collectors/
    │   ├── mod.rs             # Orquestração concorrente (tokio::join!) + Pré-Filtro de exclusão
    │   ├── gupy.rs            # Coletor nativo Gupy com normalização multi-cidade e acentos
    │   └── jobspy.rs          # Coletor assíncrono via subprocesso Python (JobSpy)
    └── gemini/
        ├── mod.rs             # Módulo de Inteligência Artificial
        ├── client.rs          # Cliente HTTP REST v1beta (reqwest) com fallback resiliente
        └── prompts.rs         # Prompts de Dossiê v2 e Match dinâmico guiado pelo Perfil Editável
```

---

## 2. Design da Interface Web (Políticas e Diretrizes Anti-AI-Slop)

A interface web (servida pelo próprio binário Rust via `cargo run -- web`) foi projetada como uma **bancada de engenharia e triagem operacional (*Workbench*)**, rejeitando os padrões genéricos gerados por IA (*"AI Slop"*) e alcançando **Slop Score `0 (CLEAN)`** na auditoria automatizada (`scan_slop.py`).

### 2.1. Invariantes Rígidas Anti-AI-Slop Aplicadas

| Categoria | Padrão "AI Slop" Eliminado | Engenharia de Interface Implementada (`static/tokens.css`) |
| :--- | :--- | :--- |
| **Cores (`CLR001`–`CLR004`)** | Gradientes decorativos roxos/índigo/violeta e textos com `bg-clip-text`. | **Tema Claro Editorial por padrão** (inspirado no LinkedIn, fundo mineral `#f3f2ef`, superfícies brancas 100% opacas `#ffffff`) com **uma única cor de acento funcional** (`--accent`: Azul Profissional `#0a66c2`) e indicadores semânticos precisos inspirados no Glassdoor (`#0a7b32` para Match Score alto `>= 70%`), além de alternador instantâneo para **Tema Escuro Técnico** (`#14171c`). |
| **Superfícies (`CMP001`–`CMP004`)** | *Glassmorphism* (`backdrop-blur`), transparências difusas, `rounded-2xl` e `shadow-2xl`. | Superfícies 100% opacas delimitadas por bordas estruturais de `1px solid var(--border)` (*hairline borders*) e raios contidos (`4px` a `6px`). |
| **Layout (`LAY001`)** | Container centralizado estilo landing page (`max-w-7xl mx-auto`) onde a página inteira rola. | **Persistent App Shell (`100vh x 100vw`)**: barra de comando superior, barra lateral de filtros e barra de status inferior permanecem fixas; apenas a tabela de vagas e o inspetor lateral possuem rolagem independente. |
| **Densidade & Tipografia (`TYP001`–`TYP002`)** | Cards gigantes com espaçamento frouxo e números desalinhados. | Grid estrito de `4px`, controles de `28px` de altura, linhas de tabela de `32px` e numerais tabulares (`font-variant-numeric: tabular-nums`) para colunas de `ID`, `Match (%)`, salários e contadores. |
| **Microcopy & Ícones (`CPY001`, `ICO001`)** | Frases de efeito (*"Potencialize sua carreira"*) e emojis usados como ícones (`🚀`, `✨`). | Rótulos estritamente literais e operacionais (`"Coletar Vagas"`, `"Enriquecer Pendentes"`, `"Calcular Match ATS"`) e ícones vetoriais SVG monocromáticos (`14px`, traço `1.75px`). |
| **Acessibilidade & Movimento (`A11Y001`–`A11Y002`, `MOT001`)** | `<div onClick>` sem foco de teclado e animações lentas `transition-all`. | Controles semânticos nativos (`<button>`, `<select>`, `<input>`, `<table>`), anel `:focus-visible` de `2px solid var(--accent)` sempre visível e orçamento de transição de `120ms` com suporte a `@media (prefers-reduced-motion: reduce)`. |

### 2.2. Anatomia da Bancada Web (`http://localhost:8787`)

1. **Barra Superior de Coleta e Comandos:**
   - Busca direta por cargo/competência e localidade, com seletor `"Apenas remoto"` e checkbox `"Auto-enriquecer IA"` (marcado por padrão para já extrair metadados e gerar embeddings ao coletar novas vagas, podendo ser desmarcado para varreduras rápidas sem consumo de cota da API).
   - Ações globais: **Enriquecer Pendentes (`e`)**, **Ranking Semântico**, **Currículo & ATS (`r`)**, **Alternador de Visão Tabela/Kanban (`v`)** e **Alternador de Tema Claro/Escuro**.
2. **Barra Lateral Esquerda (Métricas SQLite & Filtros Rápidos):**
   - Contadores em tempo real do banco local (`Total`, `Enriquecidas`, `No Funil`, `Pendentes`).
   - Busca instantânea por texto (`Ctrl+K`), filtro por **Modalidade** (`Todas`, `Remoto`, `Híbrido`, `Presencial`), **Escala/Turno** (`Comercial 5x2`, `6x2`, `12x36`, `Turnos Rotativos`) e **Estágio no Funil**.
3. **Área Principal Dual (Master-Detail Split-Pane vs. Quadro Kanban):**
   - **Modo Tabela + Inspetor (Padrão):**
     - **Esquerda:** Tabela densa de vagas com seleção ativa e indicadores de Match Score / Similaridade Semântica.
     - **Direita (Inspetor de Vaga & ATS):** Metadados extraídos pela IA (modalidade, escala/turno, salário estimado, contrato, senioridade), alerta destacado de **Red Flags**, **Competências Técnicas Extraídas** (clicáveis para inserção rápida no `curriculo.md`), **Relatório Completo de Match Score & ATS** e descrição original higienizada.
   - **Modo Quadro Kanban (`v`):**
     - Visualização em 5 colunas verticais (`Descoberta`, `Salva`, `Aplicada`, `Entrevista`, `Descartada`) para gestão visual do funil de candidaturas.
4. **Gaveta / Modal Integrado de Currículo (`curriculo.md`) & ATS (`r`):**
   - Permite visualizar, editar e salvar o arquivo `curriculo.md` diretamente pelo navegador e recalcular o Match Score da vaga selecionada imediatamente.

### 2.3. Atalhos de Teclado da Interface Web

| Tecla | Ação na Bancada Web |
| :---: | :--- |
| `j` / `k` ou `↓` / `↑` | Navega entre as vagas da tabela atualizando o Inspetor lateral |
| `m` | Executa a análise profunda de **Match Score & ATS** via Gemini para a vaga selecionada |
| `s` | Avança o estágio da vaga selecionada no Kanban (`Descoberta` → `Salva` → `Aplicada` → `Entrevista` → `Descartada`) |
| `e` | Enfileira o enriquecimento em lote das vagas pendentes (`is_enriched = 0`) |
| `r` | Abre o editor interativo do `curriculo.md` & Otimização ATS |
| `v` | Alterna entre a visão **Tabela + Inspetor** e o **Quadro Kanban** |
| `Ctrl+K` / `Cmd+K` | Foca imediatamente no campo de filtragem rápida da tabela |
| `Esc` | Fecha o modal de currículo |

---

## 3. Design da Interface de Terminal (TUI com `ratatui`)

Para operação 100% em terminal via SSH ou ambientes sem navegador, o subcomando `cargo run -- tui` abre um painel interativo construído com `ratatui` e `crossterm`:

```text
┌─ Estatísticas do Banco & Filtros Ativos ───────────────────────────────────────────────┐
│  JOB HUNTER RS   Total: 13 | Enriquecidas: 13 | Salvas/Aplicadas: 2 | Filtro: [Todas]  │
└────────────────────────────────────────────────────────────────────────────────────────┘
┌─ Vagas (13) ───────────────────────┐┌─ Detalhes da Vaga (PgUp/PgDn ou u/d para rolar) ─┐
│ ▶ [ 70%] Software Engineer - SoftD ││ ID: #12 (gupy)   Status: [Salva]   Match: 70.0   │
│   [ 58%] Analista de Suporte TI    ││ Título: Software Engineer (Python)               │
│   [ -- ] Python Data Science       ││ Modalidade: Remoto | Turno: Comercial            │
│   [ -- ] Técnico de Suporte em TI  ││ ── Resumo da IA ──────────────────────────────── │
│   [ -- ] Operador de Sistemas      ││ ── Skills Obrigatórias ───────────────────────── │
│                                    ││ ── Red Flags / Alertas ───────────────────────── │
│                                    ││ ── Análise de Match & ATS ────────────────────── │
└────────────────────────────────────┘└──────────────────────────────────────────────────┘
┌─ Atalhos & Status ─────────────────────────────────────────────────────────────────────┐
│  [↑/↓ j/k] Navegar  [f] Filtro Modalidade  [s] Mudar Status  [m] Match IA  [e] Enriq.  │
└────────────────────────────────────────────────────────────────────────────────────────┘
```

- **Atalhos da TUI:**
  - `↑`/`↓` ou `j`/`k`: Navegar na lista de vagas.
  - `PgUp`/`PgDn` ou `u`/`d`: Rolar o painel direito de detalhes e análise ATS.
  - `f`: Alternar filtro de modalidade (`Todas` → `Remoto` → `Hibrido` → `Presencial`).
  - `s`: Alternar status no Kanban (`Descoberta` → `Salva` → `Aplicada` → `Entrevista` → `Descartada`).
  - `m`: Calcular Match Score e ATS via Gemini para a vaga selecionada.
  - `e`: Enriquecer vagas pendentes em lote.
  - `q` ou `Esc`: Sair restaurando o terminal.

---

## 4. Pipeline de Inteligência Artificial (Google AI Studio REST `v1beta`)

Toda comunicação com o Google AI Studio é feita via chamadas HTTP diretas (`reqwest` + `rustls-tls`) para `https://generativelanguage.googleapis.com/v1beta/models`, utilizando `responseMimeType: "application/json"` e definição estrita de estrutura JSON no `systemInstruction`:

1. **Extração Estruturada em Lote + Dossiê v2 (`extract_jobs_batch`):**
   - Processa lotes de até 5 vagas por requisição extraindo: `work_model`, `contract_type`, `seniority`, `shift_schedule`, `required_skills`, `salary_estimate_brl`, `summary`, `red_flags` e o **Dossiê v2 (`dossier`)**:
     - **Origem de cada dado (`provenance`):** marca `work_model`, `contract_type`, `seniority`, `shift_schedule` e `salary` como `explicit` (acompanhado do trecho literal copiado da vaga), `inferred` ou `absent`.
     - **Verificação Anti-Alucinação em Código (`Dossier::verify_evidence`):** valida se a citação `evidence` realmente existe na descrição original; caso não exista, descarta a citação e rebaixa `explicit` para `inferred`.
     - **O que o texto não diz (`Dossier::missing`):** calcula automaticamente as omissões da vaga (`salario`, `escala`, `modalidade`, `contrato`, `beneficios`).
     - **Seções Estruturadas (`sections`) e Alertas por Gravidade (`alerts`):** organiza `responsibilities`, `required`, `nice_to_have`, `benefits` e classifica alertas em `critical`, `warning` ou `info`.
2. **Indexação Vetorial (`generate_embedding`):**
   - Gera vetores densos `Vec<f32>` via endpoint `:embedContent` e persiste em `embedding_json` no SQLite.
3. **Análise Profunda de Compatibilidade, Veredito e 4 Eixos (`analyze_job_match_with_profile`):**
   - Injeta dinamicamente o **Perfil de Critérios do Candidato (`candidate_profile`)** (cidade base, região aceita, escalas e pretensão salarial) junto ao `curriculo.md`.
   - Devolve o **Veredito Conservador (`verdict_kind()`: `apply`, `maybe` ou `skip`)** com 3 motivos objetivos, avaliação nos **4 Eixos (`skills`, `logistics`, `seniority`, `salary`)** e **Lacunas com Ação Sugerida (`has_unlisted`, `study`, `ignore`)**.
4. **Worker Autônomo Zero-Touch (`src/automation.rs`):**
   - Roda em segundo plano executando as **Rondas de Busca Salvas (`saved_searches`)** em paralelo na Gupy e no JobSpy (`.venv`), filtrando termos excluídos (`exclude_terms`), enriquecendo lotes pendentes, disparando **Auto-Match** para vagas com similaridade semântica $\ge 62\%$ e enviando notificações desktop (`notify-send`) para vagas `apply`.

---

## 5. Deduplicação `blake3` e Busca Semântica em Memória

- **Regra de Ouro de Deduplicação (`compute_dedup_hash`):**
  - Antes de inserir qualquer vaga ou acionar a API do Gemini, gera-se o hash hexadecimal `blake3` da chave canônica normalizada:
    ```rust
    format!("{}|{}|{}", company.trim().to_lowercase(), title.trim().to_lowercase(), location.trim().to_lowercase())
    ```
  - Garantido pela restrição `dedup_hash TEXT UNIQUE NOT NULL` no SQLite (`INSERT OR IGNORE`).
- **Similaridade de Cosseno em Memória (`cosine_similarity`):**
  - Função pura em Rust que calcula $\frac{\mathbf{a} \cdot \mathbf{b}}{\|\mathbf{a}\| \|\mathbf{b}\|}$ entre o vetor do `curriculo.md` e todas as vagas enriquecidas em RAM.

---

## 6. Instalação, Configuração e Uso

### Pré-requisitos
- **Rust** (`cargo` e `rustc` Edition 2021)
- Chave de API do **Google AI Studio** (`GEMINI_API_KEY`)
- *(Opcional)* `python3` com `python-jobspy` caso deseje ativar a bridge complementar para LinkedIn/Indeed/Glassdoor além do coletor nativo da Gupy.

### Configuração

```bash
cp .env.example .env
# Edite o arquivo .env e insira sua chave GEMINI_API_KEY
```

### Comandos Disponíveis

```bash
# 1. Iniciar a Bancada Web Anti-Slop em http://localhost:8787
cargo run -- web --port 8787

# 2. Abrir a Interface de Terminal Interativa (TUI)
cargo run -- tui

# 3. Coletar vagas via CLI (Gupy + JobSpy concorrentemente)
cargo run -- search --query "Operador de Sistemas" --location "Araraquara" --limit 20
cargo run -- search --query "Python" --remote --limit 20

# 4. Enriquecer vagas pendentes em lote + gerar embeddings
cargo run -- enrich --batch-size 5

# 5. Exibir o ranking semântico (similaridade de cosseno) no terminal
cargo run -- rank --resume curriculo.md --top 15

# 6. Executar análise profunda de Match Score & ATS para uma vaga específica
cargo run -- match --id 1 --resume curriculo.md
```
