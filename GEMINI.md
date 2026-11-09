# Job Hunter RS — Documentação Técnica, Contratos REST v1beta e Arquitetura UI/TUI

Aplicação CLI, TUI (`ratatui`) e Web Workbench (`axum` + Design System **Anti-AI-Slop**) em **Rust (Edition 2021)** para coleta automatizada de vagas de emprego (Gupy + JobSpy), deduplicação determinística via hash `blake3`, enriquecimento estruturado em lote com IA, indexação vetorial em memória e análise profunda de compatibilidade curricular / ATS via Google AI Studio REST API (`v1beta`).

---

## 1. Visão Geral da Arquitetura

```text
.
├── GEMINI.md                  # Documentação técnica, contratos REST v1beta e diretrizes UI/TUI
├── README.md                  # Guia completo do projeto, design Anti-Slop e instruções de uso
├── Cargo.toml                 # Manifesto Rust (Edition 2021) e dependências
├── .env.example               # Template de variáveis de ambiente
├── curriculo.md               # Currículo base em Markdown (SCADA/TI/Dev/Araraquara-SP)
├── migrations/
│   └── 001_init.sql           # Schema relacional SQLite (tabela `jobs` e índices)
├── scripts/
│   └── jobspy_bridge.py       # Bridge Python (LinkedIn, Indeed, Glassdoor via JobSpy)
├── static/
│   ├── tokens.css             # Tokens visuais Anti-Slop (Tema Claro editorial + Tema Escuro)
│   ├── index.html             # App Shell 100vh (Master-Detail + Kanban + Editor curriculo.md)
│   └── app.js                 # Cliente REST, atalhos keyboard-first e sincronização de estado
└── src/
    ├── main.rs                # Entrypoint CLI (clap) + Interface TUI interativa (ratatui)
    ├── web.rs                 # Servidor HTTP embutido (axum) e endpoints JSON (/api/*)
    ├── config.rs              # Carregamento de variáveis de ambiente (.env)
    ├── models.rs              # Estruturas de domínio, filtros e DTOs de serialização
    ├── db.rs                  # Persistência SQLite (sqlx), deduplicação blake3 e busca vetorial
    ├── collectors/
    │   ├── mod.rs             # Orquestração concorrente (tokio::join!) de coletores
    │   ├── gupy.rs            # Coletor HTTP nativo para a Gupy (API + SSR JSON fallback)
    │   └── jobspy.rs          # Coletor assíncrono via subprocesso Python (JobSpy)
    └── gemini/
        ├── mod.rs             # Exportação do módulo de IA
        ├── client.rs          # Cliente HTTP REST v1beta (reqwest) para geração e embeddings
        └── prompts.rs         # System instructions com especificação estrita de JSON
```

---

## 2. Contratos REST do Google AI Studio (`v1beta`)

Toda comunicação com o Gemini é realizada via chamadas HTTP diretas (`reqwest`) para os endpoints REST `v1beta`, sem SDKs intermediários.

- **Base URL:** `https://generativelanguage.googleapis.com/v1beta/models`
- **Headers Obrigatórios:**
  - `x-goog-api-key: <GEMINI_API_KEY>`
  - `Content-Type: application/json`

### 2.1. Geração e Extração Estruturada (`:generateContent`)
- **Endpoint:** `POST https://generativelanguage.googleapis.com/v1beta/models/{model}:generateContent`
- **Modelos e Fallbacks Automáticos:**
  - Extração de metadados estruturados em lote (5 a 10 vagas por requisição): `gemini-2.5-flash-lite` (com fallback automático para `gemini-3.5-flash-lite` / `gemini-3.1-flash-lite`).
  - Análise profunda de compatibilidade curricular (Match Score / ATS): `gemini-2.5-flash` (com fallback automático para `gemini-3.5-flash` / `gemini-3.6-flash`).
- **Estratégia de JSON Estrito (Compatibilidade OpenAPI 3.0):**
  - Configura-se `"responseMimeType": "application/json"` em `generationConfig` (com `"temperature": 0.1`).
  - A definição exata da estrutura JSON esperada é injetada diretamente em `systemInstruction.parts[0].text`, evitando erros `400 Bad Request` causados por propriedades como `$schema` ou `additionalProperties`.

### 2.2. Geração de Embeddings (`:embedContent`)
- **Endpoint:** `POST https://generativelanguage.googleapis.com/v1beta/models/{model}:embedContent`
- **Modelos:** `text-embedding-004` (com fallback automático para `gemini-embedding-001` / `gemini-embedding-2`).
- **Caminho de Resposta:** `response["embedding"]["values"]` -> `Vec<f32>`.

---

## 3. Diretrizes de Design da Interface Web (Anti-AI-Slop) e TUI

1. **Web Workbench (`src/web.rs` + `static/`):**
   - Auditado pelo scanner `scan_slop.py` com **Slop Score `0 (CLEAN)`**.
   - **Persistent App Shell (`100vh`):** Barra superior de coleta, barra lateral de métricas/filtros e barra de status inferior permanecem estáticas; apenas a tabela de vagas (`32px`/linha) e o Inspetor de Vaga & ATS rolam independentemente.
   - **Tema Claro Editorial por padrão (`[data-theme="light"]`) + Tema Escuro (`[data-theme="dark"]`):** Superfícies 100% opacas (`--bg-surface`), bordas de `1px solid var(--border)`, cor única de acento funcional (`#0a66c2`) e numerais tabulares (`font-variant-numeric: tabular-nums`).
   - **Visão Dual Tabela / Kanban (`v`)** e **Gaveta Interativa de Currículo & ATS (`r`)**.
2. **Terminal UI (`ratatui` + `crossterm` em `src/main.rs`):**
   - Layout dividido em 3 faixas verticais e 2 colunas horizontais (Lista de Vagas `[Score] Título - Empresa (Modalidade | Turno)` + Painel de Detalhes rolável com Resumo IA, Skills, Escala, Red Flags e Relatório ATS).
