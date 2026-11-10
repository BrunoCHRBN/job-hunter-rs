# Proposta: Job Hunter público na Vercel com BYOK e sessões isoladas

**Data:** 2026-11-10  
**Status:** proposta de arquitetura; nenhuma alteração de produção foi implementada neste documento.  
**Pergunta:** é possível publicar o Job Hunter na Vercel para outras pessoas, com *Bring Your Own Key* e separação simples de sessões?

## 1. Veredito

**Sim, tecnicamente — mas não com um deploy direto do código atual.**

A Vercel de 2026 oferece três recursos que tornam o caminho viável:

- **Rust Runtime** para funções Rust via `vercel_runtime`;
- **Container Runtime** para rodar um servidor HTTP em imagem OCI;
- **Daemons** para workers contínuos.

O caminho de menor reescrita é **Container Runtime + PostgreSQL externo + Daemon opcional**, mantendo o Axum como servidor HTTP. Porém, antes de abrir ao público, é obrigatório resolver um problema maior que o runtime: **hoje o projeto é explicitamente single-user**. Todos os visitantes compartilhariam `jobs.db`, `curriculo.md`, perfil, chave Gemini, cache de IA, worker e stream de execuções.

A separação simples por sessão anônima é possível. BYOK também é possível. O que **não** é simples é manter automação em segundo plano sem armazenar a chave do usuário no servidor; para o MVP público, a decisão correta é **desativar o worker autônomo** e aceitar apenas ações iniciadas pelo usuário com a aba aberta.

---

## 2. Evidências da plataforma

Consultas à documentação oficial da Vercel em 2026-11-10:

| Restrição / recurso | Valor oficial | Consequência para o projeto |
| :--- | :--- | :--- |
| Runtimes oficiais | Node.js, Bun, Python, **Rust**, Go, Ruby, Wasm, Edge, **Container** | Rust é suportado, mas não como processo Axum arbitrário no runtime Rust padrão |
| Rust Runtime | Beta; entrada via `vercel_runtime` + `service_fn`, um arquivo em `api/` por função | 47 rotas Axum teriam de ser reescritas ou agrupadas num handler |
| Container Runtime | Beta; imagem OCI com servidor HTTP; porta padrão `80`, configurável por `PORT` | Permite preservar o `Router` Axum com menos reescrita |
| Filesystem | Somente leitura, com `/tmp` gravável de até **500 MB** | `jobs.db` e `curriculo.md` não são duráveis |
| Escala do container | Reduz a zero após **5 min** sem tráfego em produção; `SIGTERM` com 30 s | Estado em memória e worker local não sobrevivem |
| Duração de Functions | Hobby: **300 s**; Pro/Enterprise: **800 s**, com **1800 s** em beta | Coleta, enriquecimento e SSE não podem ser infinitos |
| Corpo de requisição/resposta | **4,5 MB**; respostas streamed não têm esse limite de corpo | Upload de PDF precisa limite explícito; a lista atual de ~482 KB cabe |
| Armazenamento persistente | Vercel Blob, Global Config, Runtime Cache e Marketplace: Neon, Upstash, Supabase | Usar Postgres/Redis/Blob externos, não arquivos locais |
| Daemons | Public Beta, produção, todos os planos dentro de cotas; contínuos, mas com armazenamento efêmero e sem autoscaling | Podem substituir o worker, desde que todo estado venha de fila/DB externos |

Fontes:

- [Vercel Functions](https://vercel.com/docs/functions)
- [Runtimes](https://vercel.com/docs/functions/runtimes)
- [Rust Runtime](https://vercel.com/docs/functions/runtimes/rust)
- [Container Runtime](https://vercel.com/docs/functions/runtimes/container)
- [Function limitations](https://vercel.com/docs/functions/limitations)
- [Daemons](https://vercel.com/docs/daemons)
- [Storage overview](https://vercel.com/docs/storage)
- [Gemini API keys](https://ai.google.dev/gemini-api/docs/api-key?hl=en)

A documentação do Gemini também afirma: **“Never expose keys client-side in production”** e recomenda um backend proxy. Em BYOK, a chave é do próprio usuário, mas continua transitando pelo navegador; por isso ela deve ficar apenas em memória de sessão e ser enviada a um proxy same-origin, nunca embutida no bundle.

---

## 3. O que impede o deploy direto

| Bloqueador | Estado atual | Risco público |
| :--- | :--- | :--- |
| Identidade | Nenhum middleware de auth/cookie/sessão; `build_router()` em `src/web.rs` | Qualquer visitante lê e altera tudo |
| Banco | `DATABASE_URL=sqlite://jobs.db`, pool único em `src/db.rs` | Usuários compartilham vagas e sofrem `database is locked` |
| Unicidade | `dedup_hash UNIQUE` global | Uma vaga coletada por A impede inserção por B |
| Perfil | `candidate_profile WHERE id = 1` | Um usuário reescreve critérios de todos |
| Currículo | `PUT /api/resume` faz `fs::write(curriculo.md)` | Vazamento de PII e sobrescrita entre sessões |
| Chave Gemini | `GEMINI_API_KEY` lida uma vez em `AppConfig::from_env()` e guardada em `WebState` | Cota única e impossibilidade de BYOK |
| Cache IA | Chave Blake3 somente por prompt, sem `session_id` | Resposta/embedding do currículo de A pode servir a B |
| Worker | `tokio::spawn` + intervalo de 30 s em `src/automation.rs` | Um ciclo global; morre ao escalar a zero |
| Execuções | `broadcast::Sender` global e cancelamento por ID sem dono | Eventos de todos chegam a todos; A pode cancelar execução de B |
| SSE | Um `EventSource` global no frontend | Fluxo longo e sem filtro de sessão |
| Python | `Command::new(python_bin)` para `jobspy_bridge.py` | Depende de filesystem/processo do container; sem timeout robusto |
| Notificações | `notify-send`, URL fixa `http://localhost:8787` | Sem sentido no servidor público |
| Servidor | Bind `0.0.0.0`, sem TLS, rate limit, CORS ou headers de segurança | Exposição insegura |

A documentação do próprio projeto já resume a intenção atual: “A aplicação não possui autenticação multiusuário” em `docs/INSTALACAO_WINDOWS.md`.

---

## 4. Arquitetura mínima recomendada

### 4.1. Componentes

1. **Frontend estático**: `static/` servido pela Vercel.
2. **API container**: a mesma árvore de rotas Axum, compilada em imagem OCI, escutando `$PORT`.
3. **Neon/Supabase Postgres**: todas as tabelas com `session_id`.
4. **Upstash Redis** opcional: rate limit, deduplicação curta e locks distribuídos.
5. **Vercel Blob** opcional: PDFs e Markdown grandes, sempre com chave prefixada por `session_id`.
6. **Daemon + fila** apenas na fase 2: coleta/enriquecimento assíncronos.
7. **Sem `GEMINI_API_KEY` global** nas variáveis de ambiente da Vercel.

O Container Runtime foi escolhido em vez de dividir 47 rotas em arquivos `api/*.rs`: ele reduz a reescrita do Axum e evita o limite de funções por deployment. O Rust Runtime continua sendo uma opção futura se o backend virar um conjunto pequeno de handlers.

### 4.2. Sessão anônima simples

- No primeiro acesso, middleware cria `jh_sid`: UUID v4 assinado, `HttpOnly`, `Secure`, `SameSite=Lax`, `Path=/`, TTL de 24–72 h.
- O cookie identifica apenas a sessão; não contém chave, currículo ou dados pessoais.
- Cada request resolve `session_id` e injeta `SessionContext` no handler.
- Toda query e todo nome de blob são prefixados por `session_id`.
- Um botão **“Encerrar sessão”** apaga cookie, linhas do banco e blobs.
- Para um MVP aberto, adicionar senha de acesso ou magic link e rate limit por IP; cookie anônimo sozinho não é autenticação forte.

### 4.3. BYOK sem persistência da chave

1. A interface ganha um campo **“Chave Gemini”**.
2. A chave fica em `sessionStorage` — sobrevive a F5, desaparece ao fechar a aba.
3. Somente chamadas que usam IA enviam `X-Gemini-Key`.
4. O proxy valida formato, chama `generativelanguage.googleapis.com` e **não grava** a chave em DB, log, cookie ou arquivo.
5. Respostas usam `Cache-Control: no-store`; headers nunca entram em logs da aplicação.
6. Se ausente, endpoints de IA retornam `428 Precondition Required` com mensagem clara.
7. A UI recomenda chave restrita ao Gemini API, com alerta de cota e link para revogação.

**Limite ético e técnico:** o proxy da Vercel vê a chave durante a requisição. BYOK aqui significa “nenhuma chave compartilhada da plataforma e nenhuma persistência”, não “servidor zero-knowledge”. Chamar o Gemini diretamente do navegador eliminaria o proxy, mas contraria a recomendação oficial de segurança do Google e depende de CORS/origem restrita; só faria sentido em protótipo descartável.

**Consequência importante:** com chave apenas em `sessionStorage`, o worker autônomo não pode rodar quando a aba fecha. Para automação real em segundo plano seria preciso armazenar a chave cifrada por sessão em KMS/Secret Manager, com consentimento explícito, rotação e revogação. Isso deixa de ser “simples” e não pertence ao MVP.

---

## 5. Mudanças necessárias no código

### 5.1. Camada de sessão

- Adicionar middleware Axum que valida/renova `jh_sid`.
- Trocar `State<Arc<WebState>>` por um contexto por request: `State<Arc<AppState>> + Extension<SessionContext>`.
- `AppState` passa a conter apenas Postgres, Redis, HTTP client, configuração pública e segredo de assinatura de cookie — **sem chave Gemini**.
- Proteger `GET /api/jobs/:id`, cancelamento e retry com cláusula `WHERE session_id = $1`.
- Rate limit por IP e por sessão antes de coleta, IA e upload.

### 5.2. Persistência

Migrar SQLite para Postgres gerenciado. Todas as tabelas ganham `session_id UUID NOT NULL`:

- `sessions`, `resumes`, `candidate_profiles`, `jobs`, `job_sources`;
- `companies`, `company_aliases`, `company_sources`, `collector_configs`, `saved_searches`;
- `executions`, `execution_stages`, `execution_events`, `execution_metrics`;
- `ai_cache`, `ai_operations`.

Exemplos de invariantes:

```sql
UNIQUE (session_id, dedup_hash);
UNIQUE (session_id, normalized_name);
UNIQUE (session_id, job_id, source, external_id);
CREATE INDEX jobs_session_score_idx ON jobs (session_id, match_score DESC);
```

O perfil deixa de ser `WHERE id = 1`. O cache de IA passa a usar `blake3(session_id || op_type || prompt)`. O currículo sai de `fs::write(curriculo.md)` e vira linha `resumes(session_id, content, updated_at)` ou objeto privado no Blob. Todo job de execução carrega `session_id`.

### 5.3. Rotas e tarefas longas

- Coleta, enriquecimento, rank, match, scan e consolidação deixam de executar dentro da request.
- Padrão: `POST /api/jobs` → `202 { execution_id }` → `GET /api/executions/:id` por polling.
- Fila em Postgres/Upstash; Daemon consome tarefas idempotentes.
- SSE é substituído por polling de 2–5 s no MVP. Se mantido, precisa ser um stream curto, autenticado e filtrado por sessão.
- Timeout e limite de concorrência explícitos para os coletores.
- `notify-send` vira notificação in-app gravada no banco.
- `open_browser`, TUI e `curriculo.md` local ficam fora da imagem cloud; criar entrypoint `web --cloud`.

### 5.4. Imagem e configuração

Esboço, não implementado:

```dockerfile
FROM rust:1-bookworm AS build
WORKDIR /app
COPY . .
RUN cargo build --release --bin job-hunter

FROM debian:bookworm-slim
COPY --from=build /app/target/release/job-hunter /usr/local/bin/
COPY static/ /app/static/
# Python + requirements apenas se JobSpy for mantido no MVP.
ENV JOB_HUNTER_MODE=cloud
CMD ["job-hunter", "web", "--cloud"]
```

`web --cloud` deve:

- ler `PORT` e fazer bind em `0.0.0.0:$PORT`;
- usar `DATABASE_URL` externo;
- não abrir navegador;
- não iniciar worker local;
- aplicar headers de segurança e `no-store` nas rotas de dados;
- encerrar graciosamente em `SIGTERM`.

`vercel.json` precisaria declarar o serviço Container e, na fase 2, o Daemon. A configuração exata deve ser validada com `vercel dev` e um preview antes de produção, porque Container Runtime e Daemons estão em beta.

---

## 6. Plano em três fases

### Fase 0 — prova de isolamento, sem Vercel

1. Refatorar `WebState` para remover a chave global.
2. Introduzir `session_id` e cookie assinado.
3. Mover currículo/perfil para tabelas por sessão.
4. Testar com dois navegadores e dois cookies.
5. Desativar worker, SSE, JobSpy e automação no modo cloud.

**Objetivo:** provar que A nunca vê dados de B antes de lidar com deploy.

### Fase 1 — MVP público na Vercel

- Container Runtime + Neon Postgres.
- Static assets servidos pela Vercel.
- BYOK em `sessionStorage` + proxy para Gemini.
- Funcionalidades: coletar via Gupy, listar, filtrar, detalhar vagas, editar currículo/perfil, calcular match sob demanda.
- Rate limit, TTL e botão de apagar sessão.
- Sem login, sem daemon, sem automação, sem PDF e sem JobSpy.
- Limite de corpo de 4,5 MB aplicado ao upload.

### Fase 2 — tarefas assíncronas

- Fila + Daemon para coleta/enriquecimento.
- JobSpy empacotado no container, com timeout/concorrência.
- Polling de execuções por sessão.
- Notificações in-app.
- Upload de PDF para Blob privado.

### Fase 3 — operação pública

- Login leve ou código de acesso.
- Cotas por sessão, observabilidade e alertas de abuso.
- Retenção configurável e exclusão automática.
- Revisão dos termos dos sites consultados.
- Testes de carga e custo.

---

## 7. Critérios de aceitação e testes por mutação

O deploy só pode ser considerado seguro quando estes testes falharem ao reintroduzir o defeito:

| Teste | Mutação que deve derrubá-lo |
| :--- | :--- |
| A cria vaga; B não a vê | Remover `session_id` do `WHERE` |
| A altera perfil; B mantém o dele | Voltar `WHERE id = 1` |
| A salva currículo; B lê o próprio | Voltar `fs::write(curriculo.md)` |
| A usa chave X; B usa chave Y | Resolver chave global de `AppConfig` |
| Cache de A não responde a B | Remover `session_id` da chave Blake3 |
| A não cancela execução de B | Remover checagem de dono |
| Eventos de execução são filtrados | Voltar `broadcast::Sender` global |
| Reinício preserva dados | Gravar em `/tmp` em vez de Postgres |
| Request sem chave retorna 428 | Usar chave global de fallback |
| Cookie inválido é rejeitado | Aceitar `jh_sid` sem assinatura |

Também medir, com dois navegadores reais: resposta `Set-Cookie`, ausência da chave em logs/DB, isolamento após restart, e comportamento ao expirar a sessão.

---

## 8. Alternativa mais simples fora da Vercel

Se a prioridade é publicar logo o **Axum atual**, uma plataforma de container com volume persistente — Fly.io, Railway, Render ou Cloud Run — exige menos mudanças de runtime: um processo longo, `jobs.db` em volume e `PORT` configurável.

Mesmo nesse caminho, **BYOK e múltiplos usuários ainda exigem a refatoração de sessão**: banco compartilhado, currículo em arquivo, perfil `id=1`, cache e worker global continuam inaceitáveis. A vantagem é apenas eliminar a conversão para funções/daemon e a dependência de armazenamento externo imediato.

Uma opção intermediária é **uma instância isolada por sessão** criada sob demanda por um orquestrador externo. É mais segura por construção, mas tem custo, latência de provisionamento e complexidade de limpeza; não é a opção mais simples para um MVP.

---

## 9. Decisão recomendada

1. **Não publicar o estado atual.** O risco não é indisponibilidade: é vazamento cruzado de currículo, chave, cache e execuções.
2. Para uma URL pública na Vercel, adotar **Container Runtime + sessão anônima assinada + Postgres por `session_id` + BYOK efêmero via proxy**.
3. Começar pela **Fase 0** e pelo **MVP da Fase 1**, com o worker desligado.
4. Só habilitar automação em segundo plano após uma decisão explícita sobre armazenamento cifrado das chaves; sem isso, BYOK e daemon são requisitos conflitantes.
5. Tratar Container Runtime, Rust Runtime e Daemons como beta: validar limites, cotas e preço em um preview antes de promover.

**Resumo em uma frase:** dá para rodar na Vercel em 2026, mas o trabalho não é “subir o servidor”; é transformar um workbench local single-user num serviço stateless, particionado por sessão, com estado externo e chave do usuário apenas em trânsito.

