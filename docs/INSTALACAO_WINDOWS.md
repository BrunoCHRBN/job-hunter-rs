# Instalação do Job Hunter RS no Windows

Este guia instala o **Job Hunter RS** em um desktop Windows que ainda não possui Rust, compilador C++, Git, Python ou qualquer ambiente de desenvolvimento. Ele cobre a instalação nativa no Windows 10/11 e uma alternativa isolada com WSL2.

O resultado é uma aplicação local, acessada em `http://localhost:8787`, com banco SQLite e currículo em Markdown mantidos na própria máquina.

---

## 1. Escolha do caminho

| Caminho | Indicado quando | Vantagens | Limitações |
| :--- | :--- | :--- | :--- |
| **Nativo no Windows** (recomendado) | O objetivo é usar a aplicação como ferramenta desktop | Sem virtualização; abre no navegador padrão; dados em uma pasta comum do Windows | Exige Microsoft C++ Build Tools e Rust |
| **WSL2 + Ubuntu** | A virtualização está disponível e você prefere um ambiente Linux reproduzível | Instalação por linha de comando mais simples; maior paridade com Linux | Consome mais RAM/disco; o navegador deve ser aberto manualmente em `localhost` |

Os dois caminhos usam o mesmo código e o mesmo banco SQLite. Não é necessário instalar os dois.

### O que será instalado

**Caminho nativo:**

1. Microsoft Visual C++ Build Tools, com o workload de C++ e o Windows SDK.
2. Rust via `rustup`, usando o toolchain MSVC.
3. Git para Windows — ou o download do repositório em ZIP.
4. Python 3.12 — opcional, somente para LinkedIn, Indeed e Glassdoor via JobSpy.
5. Job Hunter RS compilado em `target\release\job-hunter.exe`.

**Caminho WSL2:**

1. WSL2 e Ubuntu.
2. `build-essential`, Git e Python no Ubuntu.
3. Rust via `rustup`.
4. Job Hunter RS compilado dentro do WSL.

---

## 2. Requisitos do sistema

| Item | Mínimo | Recomendado |
| :--- | :--- | :--- |
| Sistema | Windows 10 22H2 ou Windows 11, 64 bits | Windows 11 atualizado |
| Processador | x86-64 com 2 núcleos | 4 núcleos ou mais |
| Memória | 4 GB | 8 GB ou mais |
| Disco livre | 6 GB | 10 GB ou mais |
| Internet | Necessária para instalar e compilar | Banda larga |
| Permissão | Administrador para instalar as ferramentas | Execução normal do app, sem administrador |
| Navegador | Edge, Chrome ou Firefox atualizado | Edge já incluído no Windows |

O aplicativo em si é leve. O espaço adicional vem principalmente do compilador C++ e dos arquivos intermediários de compilação.

> **Privacidade:** `jobs.db`, `.env` e `curriculo.md` são dados locais. O `.env` contém sua chave do Gemini e nunca deve ser enviado ao GitHub.

---

## 3. Instalação nativa no Windows

Execute os passos na ordem. Abra o **PowerShell** pelo menu Iniciar. Para instalar ferramentas, use uma sessão como administrador; para compilar e executar o app, uma sessão normal é suficiente.

### 3.1. Atualizar o Windows

1. Abra **Configurações → Windows Update**.
2. Instale todas as atualizações pendentes.
3. Reinicie o computador.

Isso reduz falhas de instalação do Windows SDK e do compilador.

### 3.2. Instalar o Microsoft C++ Build Tools

O Rust no Windows usa o linker e as bibliotecas C++ da Microsoft.

1. Acesse [Visual Studio Build Tools](https://visualstudio.microsoft.com/visual-cpp-build-tools/).
2. Baixe **Build Tools for Visual Studio**.
3. Execute o instalador e escolha **Desenvolvimento para desktop com C++**.
4. No painel direito, confirme estes componentes:
   - **MSVC v143 — VS 2022 C++ x64/x86 build tools**
   - **Windows 10 SDK** ou **Windows 11 SDK**
   - **C++ CMake tools for Windows** (recomendado)
5. Conclua a instalação e reinicie o Windows.

Se o instalador já estiver presente, abra **Visual Studio Installer → Modificar** e acrescente o workload ausente.

### 3.3. Instalar o Rust

1. Acesse [rust-lang.org/tools/install](https://www.rust-lang.org/tools/install).
2. Baixe `rustup-init.exe` para Windows 64 bits.
3. Execute o arquivo e aceite a opção padrão:

```text
1) Proceed with standard installation
```

4. Ao final, feche e abra novamente o PowerShell.
5. Confirme:

```powershell
rustc --version
cargo --version
```

Ambos devem responder com uma versão. O host esperado é `x86_64-pc-windows-msvc`.

> Se o `rustup` informar que falta o linker, confirme o workload de C++ no Build Tools. Em último caso, abra **Developer PowerShell for VS 2022** e execute os comandos de compilação por ele.

### 3.4. Instalar o Git

1. Acesse [git-scm.com/download/win](https://git-scm.com/download/win).
2. Baixe e execute o instalador de 64 bits.
3. Mantenha as opções padrão, incluindo **Git from the command line and also from 3rd-party software**.
4. Feche e reabra o PowerShell.
5. Confirme:

```powershell
git --version
```

**Alternativa sem Git:** baixe o repositório como ZIP em [github.com/BrunoCHRBN/job-hunter-rs](https://github.com/BrunoCHRBN/job-hunter-rs), use **Code → Download ZIP**, extraia e abra o PowerShell dentro da pasta extraída. Nesse caso, atualizações futuras exigem baixar um novo ZIP.

### 3.5. Obter o projeto

Escolha uma pasta sem acentos e sem OneDrive para evitar bloqueios de arquivo durante a compilação. Exemplo:

```powershell
New-Item -ItemType Directory -Force "$HOME\Projetos" | Out-Null
Set-Location "$HOME\Projetos"
git clone https://github.com/BrunoCHRBN/job-hunter-rs.git
Set-Location .\job-hunter-rs
```

Confirme que você está na raiz correta:

```powershell
Get-ChildItem Cargo.toml, README.md, scripts\jobspy_bridge.py
```

Os três itens devem existir.

### 3.6. Instalar Python e JobSpy (opcional)

Sem Python, o aplicativo funciona com o coletor nativo da Gupy. Python é necessário apenas para consultar LinkedIn, Indeed e Glassdoor pela bridge JobSpy.

1. Acesse [python.org/downloads/windows](https://www.python.org/downloads/windows/).
2. Baixe **Python 3.12, Windows installer (64-bit)**.
3. No instalador, marque **Add python.exe to PATH** e conclua.
4. Feche e reabra o PowerShell.
5. Confirme o lançador:

```powershell
py -3.12 --version
```

Crie o ambiente virtual e instale as dependências:

```powershell
py -3.12 -m venv .venv
.\.venv\Scripts\python.exe -m pip install --upgrade pip
.\.venv\Scripts\python.exe -m pip install -r requirements.txt
```

Teste a importação:

```powershell
.\.venv\Scripts\python.exe -c "from jobspy import scrape_jobs; print('JobSpy OK')"
```

Se aparecer `JobSpy OK`, a integração opcional está pronta. Se a instalação de `numpy`, `pandas` ou `curl_cffi` falhar, confirme o workload C++ da etapa 3.2 e tente novamente.

### 3.7. Configurar o `.env`

Crie o arquivo local a partir do modelo:

```powershell
Copy-Item .env.example .env
notepad .env
```

Preencha os valores:

```dotenv
GEMINI_API_KEY="SUA_CHAVE_GEMINI_AQUI"
DATABASE_URL="sqlite://jobs.db"
PYTHON_BIN=".venv/Scripts/python.exe"
RESUME_PATH="curriculo.md"
RUST_LOG="info"
```

- **`GEMINI_API_KEY`**: crie em [Google AI Studio](https://aistudio.google.com/app/apikey). A chave é necessária para enriquecimento, ranking semântico e Match/ATS. Sem ela, a interface abre e a coleta nativa continua, mas os recursos de IA falham com uma mensagem explícita.
- **`PYTHON_BIN`**: use barras normais, como acima. Se você não instalou Python, deixe a linha comentada com `#`; o app continuará com a Gupy.
- **`DATABASE_URL`**: mantenha `sqlite://jobs.db` para criar o banco na raiz do projeto.
- **`RESUME_PATH`**: caminho do currículo Markdown usado nas análises.

Não coloque espaços ao redor de `=` e não publique o `.env`.

### 3.8. Compilar

Na raiz do projeto:

```powershell
cargo build --release
```

A primeira compilação baixa e compila todas as dependências; pode levar de 5 a 20 minutos. As seguintes são incrementais e bem mais rápidas.

O executável será criado em:

```text
target\release\job-hunter.exe
```

Confirme:

```powershell
Get-Item .\target\release\job-hunter.exe
```

Se houver erro de linker ou de `link.exe`, consulte a seção de solução de problemas antes de continuar.

### 3.9. Executar

Com o arquivo `.env` na raiz do projeto:

```powershell
.\target\release\job-hunter.exe web --port 8787
```

Abra manualmente [http://localhost:8787](http://localhost:8787). Mantenha o PowerShell aberto enquanto usa o app; `Ctrl+C` encerra o servidor.

Também é possível iniciar sem subcomando:

```powershell
.\target\release\job-hunter.exe
```

Nesse modo, o Job Hunter sobe na porta `8787` e tenta abrir o navegador padrão automaticamente. Se o navegador não abrir, acesse a URL manualmente.

Para iniciar em segundo plano, sem deixar a janela do terminal ocupando espaço:

```powershell
Start-Process .\target\release\job-hunter.exe -ArgumentList "web", "--port", "8787"
```

Para encerrar essa instância depois:

```powershell
Get-Process job-hunter -ErrorAction SilentlyContinue | Stop-Process
```

### 3.10. Validar a instalação

Com o servidor ativo, execute em outro PowerShell:

```powershell
Invoke-RestMethod http://127.0.0.1:8787/api/stats
```

A resposta deve conter `total_jobs`, `enriched_jobs` e `saved_or_applied_jobs`. Na interface, confirme:

1. O Painel abre sem erro.
2. **Vagas** mostra a lista e permite abrir o detalhe.
3. **Empresas** carrega a lista ao entrar na visão.
4. **Currículo** permite editar e salvar o Markdown.
5. O tema claro/escuro alterna.
6. Uma coleta de teste retorna vagas da Gupy, mesmo sem Python.

Para testar a suíte automatizada na máquina:

```powershell
cargo test
```

### 3.11. Primeiro uso

1. Abra **Currículo** e substitua o conteúdo pelo seu currículo real.
2. Abra **Painel** e ajuste as buscas salvas e o perfil de critérios.
3. Execute uma coleta pela interface.
4. Use **Calcular match** em uma vaga para validar a chave do Gemini.
5. Se instalou Python, ative os coletores de LinkedIn/Indeed/Glassdoor e acompanhe possíveis bloqueios anti-bot no histórico de execuções.

O arquivo `curriculo.md` é dado pessoal local. Não faça commit dele em um repositório público.

---

## 4. Atualização, backup e remoção

### 4.1. Atualizar o aplicativo

Pare o servidor com `Ctrl+C` e faça backup do banco antes de atualizar:

```powershell
Copy-Item .\jobs.db ".\jobs-backup-$(Get-Date -Format yyyyMMdd-HHmm).db"
git pull --ff-only
cargo build --release
```

As migrações SQL pendentes são aplicadas automaticamente na próxima inicialização. Se o `git pull` recusar por alterações locais, não use `git reset --hard`: preserve suas mudanças com `git stash` ou copie o projeto para outra pasta antes de decidir.

### 4.2. Fazer backup

O estado útil está em:

| Arquivo | Conteúdo |
| :--- | :--- |
| `jobs.db` | Vagas, empresas, perfis, buscas e execuções |
| `curriculo.md` | Currículo usado nas análises |
| `.env` | Chave e configuração local |

Pare o app antes de copiar o banco:

```powershell
New-Item -ItemType Directory -Force .\backups | Out-Null
Copy-Item .\jobs.db ".\backups\jobs-$(Get-Date -Format yyyyMMdd-HHmm).db"
Copy-Item .\curriculo.md ".\backups\curriculo-$(Get-Date -Format yyyyMMdd-HHmm).md"
```

Não copie `.env` para locais compartilhados. Guarde a chave em um gerenciador de senhas.

### 4.3. Remover o aplicativo

1. Encerre o processo com `Get-Process job-hunter | Stop-Process`.
2. Apague a pasta do projeto, incluindo `jobs.db`, `.venv` e `target`.
3. Revogue a chave do Gemini no Google AI Studio, se não for mais usá-la.
4. Rust, Python e Build Tools podem ser removidos em **Aplicativos instalados** se não forem usados por outros projetos.

O Job Hunter não instala serviço, driver ou tarefa agendada; a remoção da pasta é suficiente para o aplicativo.

---

## 5. Segurança e rede

- O servidor escuta em todas as interfaces locais na porta escolhida. O Windows pode exibir um aviso do Firewall; permita somente **Redes privadas** se precisar acessar de outro dispositivo confiável. Para uso individual, o acesso continua funcionando por `localhost`.
- **Não** faça port forwarding, túnel público ou exposição direta da porta `8787` na internet. A aplicação não possui autenticação multiusuário.
- A chave do Gemini fica em `.env`; o `.gitignore` já exclui esse arquivo.
- Scrapers e APIs de terceiros têm termos de uso e limites próprios. Use volumes moderados.
- O banco contém currículo e histórico de candidaturas. Trate `jobs.db` como dado pessoal.

---

## 6. Solução de problemas

| Sintoma | Causa provável | Correção |
| :--- | :--- | :--- |
| `cargo` ou `rustc` não reconhecido | PATH ainda não atualizado | Feche e reabra o PowerShell; se persistir, reinstale o Rust e reinicie |
| `linker link.exe not found` | Build Tools ou workload C++ ausente | Instale **Desenvolvimento para desktop com C++** e o Windows SDK; use o Developer PowerShell se necessário |
| `Microsoft Visual C++ 14.0 or greater is required` | Compilador C++ ausente ao instalar pacotes Python | Instale o Build Tools da etapa 3.2 e repita `pip install -r requirements.txt` |
| `python` abre a Microsoft Store | Alias do Windows, não o Python real | Instale Python pelo python.org, marque **Add to PATH** e use `py -3.12` |
| `No module named jobspy` | Dependências instaladas fora do `.venv` | Execute `.\.venv\Scripts\python.exe -m pip install -r requirements.txt` |
| `PYTHON_BIN` não encontrado | Caminho Unix usado no Windows | Use `.venv/Scripts/python.exe` ou remova a linha para autodetecção |
| Porta 8787 ocupada | Outra instância ou processo | `Get-NetTCPConnection -LocalPort 8787`; encerre o processo ou use `--port 8788` |
| Navegador não abre | Associação de URL ou execução em WSL | Abra `http://localhost:8787` manualmente |
| Firewall bloqueia acesso de outro PC | Regra de entrada ausente | Permita apenas rede privada; não exponha à internet |
| Gemini retorna 429 | Cota temporária esgotada | Aguarde a renovação, reduza o lote ou use outra chave |
| `database is locked` | Duas instâncias usando o mesmo SQLite | Encerre todas e abra apenas uma instância |
| JobSpy retorna zero vagas | Bloqueio anti-bot ou filtro muito restrito | Veja o diagnóstico por fonte; amplie localidade/prazo; tente novamente mais tarde |
| Build muito lento ou sem espaço | Primeira compilação e cache do Cargo | Libere pelo menos 6 GB; não interrompa a primeira compilação |
| SmartScreen alerta sobre o `.exe` | Binário local não assinado | Use **Mais informações → Executar assim mesmo** somente se compilou o código desta fonte |

Para obter logs mais detalhados:

```powershell
$env:RUST_LOG="debug"
.\target\release\job-hunter.exe web --port 8787
```

---

## 7. Alternativa com WSL2

Use este caminho se a instalação nativa falhar ou se você quiser um ambiente Linux isolado. O WSL2 requer virtualização habilitada na BIOS/UEFI e Windows 10 versão 2004 ou superior.

### 7.1. Instalar o WSL2 e Ubuntu

Abra o PowerShell como administrador:

```powershell
wsl --install -d Ubuntu
```

Reinicie o Windows quando solicitado. Na primeira abertura do Ubuntu, crie um usuário e uma senha Linux. Depois, no PowerShell:

```powershell
wsl --set-default-version 2
wsl --list --verbose
```

A distribuição Ubuntu deve aparecer com versão `2`.

Se `wsl --install` não estiver disponível, siga a [documentação oficial da Microsoft](https://learn.microsoft.com/windows/wsl/install) e instale manualmente o recurso **Plataforma de Máquina Virtual** e o **Subsistema do Windows para Linux**.

### 7.2. Preparar o Ubuntu

Dentro do terminal Ubuntu:

```bash
sudo apt update
sudo apt install -y build-essential pkg-config libssl-dev git curl python3 python3-venv python3-pip
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source "$HOME/.cargo/env"
rustc --version
cargo --version
```

### 7.3. Obter, configurar e compilar

```bash
mkdir -p "$HOME/projetos"
cd "$HOME/projetos"
git clone https://github.com/BrunoCHRBN/job-hunter-rs.git
cd job-hunter-rs
python3 -m venv .venv
./.venv/bin/python -m pip install --upgrade pip
./.venv/bin/python -m pip install -r requirements.txt
cp .env.example .env
nano .env
```

No `.env`, use o formato Linux:

```dotenv
GEMINI_API_KEY="SUA_CHAVE_GEMINI_AQUI"
DATABASE_URL="sqlite://jobs.db"
PYTHON_BIN=".venv/bin/python"
RESUME_PATH="curriculo.md"
RUST_LOG="info"
```

Salve com `Ctrl+O`, `Enter` e saia com `Ctrl+X`. Depois:

```bash
cargo build --release
./target/release/job-hunter web --port 8787
```

Abra `http://localhost:8787` no navegador do Windows. O WSL2 encaminha portas locais automaticamente. A abertura automática do navegador não funciona dentro do WSL sem integração adicional; use a URL manualmente.

### 7.4. Acessar e fazer backup dos arquivos

No Explorador de Arquivos do Windows, a pasta Linux pode ser acessada por:

```text
\\wsl$\Ubuntu\home\SEU_USUARIO\projetos\job-hunter-rs
```

Para copiar o banco para o Windows:

```bash
mkdir -p /mnt/c/Users/SEU_USUARIO/Backups/job-hunter
cp jobs.db /mnt/c/Users/SEU_USUARIO/Backups/job-hunter/jobs-$(date +%Y%m%d-%H%M).db
```

Mantenha o projeto dentro do sistema de arquivos Linux (`~/projetos`), não em `/mnt/c`, para evitar lentidão de compilação e problemas de permissão.

---

## 8. Referência rápida de comandos

| Objetivo | Windows nativo | WSL2/Ubuntu |
| :--- | :--- | :--- |
| Verificar Rust | `rustc --version` | `rustc --version` |
| Compilar | `cargo build --release` | `cargo build --release` |
| Testar | `cargo test` | `cargo test` |
| Abrir web | `.\target\release\job-hunter.exe web --port 8787` | `./target/release/job-hunter web --port 8787` |
| Abrir TUI | `.\target\release\job-hunter.exe tui` | `./target/release/job-hunter tui` |
| Coletar via CLI | `.\target\release\job-hunter.exe search --query "Python" --location "São Paulo"` | `./target/release/job-hunter search --query "Python" --location "São Paulo"` |
| Banco padrão | `jobs.db` | `jobs.db` |
| URL local | `http://localhost:8787` | `http://localhost:8787` |

---

## 9. Checklist final

- [ ] `rustc --version` e `cargo --version` respondem.
- [ ] `cargo build --release` termina sem erro.
- [ ] `job-hunter.exe` existe em `target\release`.
- [ ] `.env` existe, está fora do Git e contém a chave correta.
- [ ] `cargo test` passa.
- [ ] `http://localhost:8787` abre a interface.
- [ ] `/api/stats` responde em JSON.
- [ ] O currículo foi substituído pelo conteúdo do usuário.
- [ ] Um backup de `jobs.db` foi criado antes da primeira atualização.
- [ ] A porta não está exposta à internet.

---

## 10. Limitações conhecidas desta versão

- Não há instalador `.msi` nem binário Windows assinado publicado. A instalação é feita compilando o código-fonte; o SmartScreen pode avisar sobre o executável local.
- A validação automatizada desta versão foi executada em Linux. O código inclui caminhos específicos para Windows (abertura do navegador e detecção de `.venv\Scripts\python.exe`), mas o primeiro uso em Windows deve incluir `cargo test` e o checklist acima.
- Notificações desktop do worker de automação usam `notify-send`, disponível em Linux. No Windows nativo o ciclo continua funcionando, mas sem notificação nativa; acompanhe o histórico de execuções na interface.
- A bridge JobSpy depende de sites de terceiros e pode sofrer bloqueio anti-bot, independentemente da instalação.
- O servidor não possui autenticação. Ele é destinado a uso local e individual.
- O projeto usa SQLite local; não abra duas instâncias simultâneas apontando para o mesmo `jobs.db`.

Se algum passo falhar, colete a saída completa do PowerShell, a versão do Windows (`winver`), as versões de `rustc`, `cargo`, `py` e a mensagem de erro. Isso permite diagnosticar sem adivinhação.


