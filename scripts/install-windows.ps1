#requires -Version 5.1
<#
.SYNOPSIS
    Instala as dependências e compila o Job Hunter RS no Windows.

.DESCRIPTION
    Ponto de entrada para um Windows sem Rust, Git, Python ou compilador C++.
    Instala Microsoft C++ Build Tools, Rust MSVC, Git, Python 3.12 opcional,
    cria o .env, o ambiente virtual do JobSpy e compila o executável release.

.PARAMETER InstallDir
    Pasta usada quando o script é executado fora do repositório.

.PARAMETER WithPython
    Instala Python 3.12 e as dependências do JobSpy (LinkedIn/Indeed/Glassdoor).

.PARAMETER SkipSystemTools
    Não instala Build Tools, Git ou Rust; apenas valida e compila.

.PARAMETER SkipBuild
    Prepara as dependências, mas não executa cargo build.

.PARAMETER RunTests
    Executa cargo test depois da compilação.

.PARAMETER NoPause
    Não aguarda Enter no final. Útil para automação.

.EXAMPLE
    .\scripts\install-windows.ps1 -WithPython

.EXAMPLE
    .\scripts\install-windows.ps1 -SkipSystemTools -RunTests -NoPause
#>
[CmdletBinding()]
param(
    [string]$InstallDir = (Join-Path $HOME 'Projetos\job-hunter-rs'),
    [string]$RepoUrl = 'https://github.com/BrunoCHRBN/job-hunter-rs.git',
    [switch]$WithPython,
    [switch]$SkipSystemTools,
    [switch]$SkipBuild,
    [switch]$RunTests,
    [switch]$NoPause
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
[Net.ServicePointManager]::SecurityProtocol = [Net.SecurityProtocolType]::Tls12

function Write-Step {
    param([Parameter(Mandatory = $true)][string]$Message)
    Write-Host "`n==> $Message" -ForegroundColor Cyan
}

function Write-Ok {
    param([Parameter(Mandatory = $true)][string]$Message)
    Write-Host "    OK: $Message" -ForegroundColor Green
}

function Write-Warn {
    param([Parameter(Mandatory = $true)][string]$Message)
    Write-Host "    AVISO: $Message" -ForegroundColor Yellow
}

function Test-Command {
    param([Parameter(Mandatory = $true)][string]$Name)
    return [bool](Get-Command $Name -ErrorAction SilentlyContinue)
}

function Update-SessionPath {
    $entries = @(
        [Environment]::GetEnvironmentVariable('Path', 'Machine'),
        [Environment]::GetEnvironmentVariable('Path', 'User'),
        (Join-Path $HOME '.cargo\bin')
    ) | Where-Object { -not [string]::IsNullOrWhiteSpace($_) }

    $env:Path = ($entries -join ';')
}

function Assert-Windows64 {
    if ($env:OS -ne 'Windows_NT' -or -not [Environment]::Is64BitOperatingSystem) {
        throw 'Este instalador requer Windows de 64 bits.'
    }
}

function Assert-Administrator {
    $identity = [Security.Principal.WindowsIdentity]::GetCurrent()
    $principal = New-Object Security.Principal.WindowsPrincipal($identity)
    if (-not $principal.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)) {
        throw 'Abra o PowerShell como Administrador para instalar as ferramentas do sistema.'
    }
}

function Invoke-Download {
    param(
        [Parameter(Mandatory = $true)][string]$Uri,
        [Parameter(Mandatory = $true)][string]$OutFile
    )
    Write-Host "    Baixando: $Uri"
    Invoke-WebRequest -UseBasicParsing -Uri $Uri -OutFile $OutFile
}

function Invoke-Installer {
    param(
        [Parameter(Mandatory = $true)][string]$FilePath,
        [string[]]$Arguments = @(),
        [int[]]$SuccessCodes = @(0)
    )
    $process = Start-Process -FilePath $FilePath -ArgumentList $Arguments -Wait -PassThru
    if ($SuccessCodes -notcontains $process.ExitCode) {
        throw "O instalador '$FilePath' terminou com código $($process.ExitCode)."
    }
    if ($process.ExitCode -in @(1641, 3010)) {
        Write-Warn 'O Windows precisa ser reiniciado para concluir esta instalação.'
    }
}

function Test-Winget {
    return Test-Command 'winget'
}

function Install-WingetPackage {
    param(
        [Parameter(Mandatory = $true)][string]$Id,
        [Parameter(Mandatory = $true)][string]$DisplayName
    )
    if (-not (Test-Winget)) {
        throw "winget não está disponível. Instale '$DisplayName' manualmente e execute novamente com -SkipSystemTools."
    }

    Write-Host "    Instalando $DisplayName via winget..."
    & winget install --id $Id --exact --accept-package-agreements --accept-source-agreements --disable-interactivity
    if ($LASTEXITCODE -ne 0) {
        throw "winget não conseguiu instalar '$DisplayName' (código $LASTEXITCODE)."
    }
}

function Test-BuildTools {
    $vswhere = Join-Path ${env:ProgramFiles(x86)} 'Microsoft Visual Studio\Installer\vswhere.exe'
    if (-not (Test-Path $vswhere)) {
        return $false
    }
    $installation = & $vswhere -latest -products * -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath
    return -not [string]::IsNullOrWhiteSpace(($installation | Out-String))
}

function Install-BuildTools {
    if (Test-BuildTools) {
        Write-Ok 'Microsoft C++ Build Tools já está instalado.'
        return
    }

    Write-Step 'Instalando Microsoft C++ Build Tools'
    Assert-Administrator

    $installer = Join-Path $env:TEMP 'vs_BuildTools_job_hunter.exe'
    try {
        Invoke-Download -Uri 'https://aka.ms/vs/17/release/vs_BuildTools.exe' -OutFile $installer
        Invoke-Installer -FilePath $installer -Arguments @(
            '--quiet',
            '--wait',
            '--norestart',
            '--add',
            'Microsoft.VisualStudio.Workload.VCTools',
            '--includeRecommended'
        ) -SuccessCodes @(0, 1641, 3010)
    }
    finally {
        Remove-Item $installer -Force -ErrorAction SilentlyContinue
    }

    if (-not (Test-BuildTools)) {
        throw 'Os Build Tools foram instalados, mas o workload C++ não foi detectado. Abra o Visual Studio Installer, adicione "Desenvolvimento para desktop com C++" e reinicie.'
    }
    Write-Ok 'Microsoft C++ Build Tools instalado.'
}

function Install-Git {
    if (Test-Command 'git') {
        Write-Ok 'Git já está instalado.'
        return
    }

    Write-Step 'Instalando Git para Windows'
    Assert-Administrator

    if (Test-Winget) {
        Install-WingetPackage -Id 'Git.Git' -DisplayName 'Git para Windows'
    }
    else {
        Write-Warn 'winget ausente; usando o instalador oficial do Git for Windows.'
        $headers = @{ 'User-Agent' = 'job-hunter-rs-installer' }
        $release = Invoke-RestMethod -UseBasicParsing -Headers $headers -Uri 'https://api.github.com/repos/git-for-windows/git/releases/latest'
        $asset = $release.assets | Where-Object { $_.name -match '^Git-.*-64-bit\.exe$' } | Select-Object -First 1
        if (-not $asset) {
            throw 'Não foi possível localizar o instalador oficial do Git. Instale manualmente em https://git-scm.com/download/win.'
        }

        $installer = Join-Path $env:TEMP 'Git_job_hunter.exe'
        try {
            Invoke-Download -Uri $asset.browser_download_url -OutFile $installer
            Invoke-Installer -FilePath $installer -Arguments @('/VERYSILENT', '/NORESTART', '/NOCANCEL', '/SP-')
        }
        finally {
            Remove-Item $installer -Force -ErrorAction SilentlyContinue
        }
    }

    Update-SessionPath
    if (-not (Test-Command 'git')) {
        throw 'Git foi instalado, mas não está disponível nesta sessão. Feche o PowerShell, abra novamente e repita o comando.'
    }
    Write-Ok 'Git instalado.'
}

function Install-Rust {
    $newInstall = $false
    if (-not (Test-Command 'cargo') -or -not (Test-Command 'rustc')) {
        Write-Step 'Instalando Rust (toolchain MSVC)'
        $installer = Join-Path $env:TEMP 'rustup-init-job-hunter.exe'
        try {
            Invoke-Download -Uri 'https://win.rustup.rs/x86_64' -OutFile $installer
            Invoke-Installer -FilePath $installer -Arguments @(
                '-y',
                '--default-host',
                'x86_64-pc-windows-msvc',
                '--profile',
                'default'
            )
        }
        finally {
            Remove-Item $installer -Force -ErrorAction SilentlyContinue
        }
        $newInstall = $true
        Update-SessionPath
    }

    if (-not (Test-Command 'cargo') -or -not (Test-Command 'rustc')) {
        throw 'Rust foi instalado, mas cargo/rustc não estão disponíveis. Feche o PowerShell, abra novamente e repita o comando.'
    }

    if ($newInstall -and (Test-Command 'rustup')) {
        & rustup default stable-x86_64-pc-windows-msvc
        if ($LASTEXITCODE -ne 0) {
            throw 'Não foi possível ativar o toolchain stable-x86_64-pc-windows-msvc.'
        }
    }

    $hostLine = (& rustc -vV | Select-String -Pattern '^host:' | Select-Object -First 1).ToString()
    if ($hostLine -notmatch 'windows-msvc') {
        throw "O Rust atual não usa o toolchain MSVC ($hostLine). Execute: rustup default stable-x86_64-pc-windows-msvc"
    }
    Write-Ok "Rust disponível: $(& rustc --version)"
}

function Install-Python {
    if (-not $WithPython) {
        Write-Step 'Python/JobSpy opcional não solicitado'
        Write-Host '    Use -WithPython para habilitar LinkedIn, Indeed e Glassdoor.'
        return
    }

    Write-Step 'Preparando Python 3.12 e JobSpy'
    $python = $null
    if (Test-Command 'py') {
        & py -3.12 --version *> $null
        if ($LASTEXITCODE -eq 0) {
            $python = (& py -3.12 -c 'import sys; print(sys.executable)').Trim()
        }
    }

    if (-not $python) {
        Assert-Administrator
        Install-WingetPackage -Id 'Python.Python.3.12' -DisplayName 'Python 3.12'
        Update-SessionPath
        if (Test-Command 'py') {
            & py -3.12 --version *> $null
            if ($LASTEXITCODE -eq 0) {
                $python = (& py -3.12 -c 'import sys; print(sys.executable)').Trim()
            }
        }
        if (-not $python) {
            $candidates = @(
                (Join-Path $env:LOCALAPPDATA 'Programs\Python\Python312\python.exe'),
                (Join-Path $env:ProgramFiles 'Python312\python.exe')
            )
            $python = $candidates | Where-Object { Test-Path $_ } | Select-Object -First 1
        }
    }

    if (-not $python) {
        throw 'Python 3.12 não foi encontrado. Instale em https://www.python.org/downloads/windows/ marcando "Add python.exe to PATH" e repita com -SkipSystemTools -WithPython.'
    }

    $venvPython = Join-Path $script:RepoRoot '.venv\Scripts\python.exe'
    if (-not (Test-Path $venvPython)) {
        Write-Host '    Criando ambiente virtual .venv...'
        & $python -m venv (Join-Path $script:RepoRoot '.venv')
        if ($LASTEXITCODE -ne 0) {
            throw 'Falha ao criar o ambiente virtual .venv.'
        }
    }

    & $venvPython -m pip install --upgrade pip --disable-pip-version-check
    if ($LASTEXITCODE -ne 0) {
        throw 'Falha ao atualizar o pip no .venv.'
    }
    & $venvPython -m pip install -r (Join-Path $script:RepoRoot 'requirements.txt') --disable-pip-version-check
    if ($LASTEXITCODE -ne 0) {
        throw 'Falha ao instalar requirements.txt. Confirme o workload C++ e tente novamente.'
    }

    Set-EnvValue -Path (Join-Path $script:RepoRoot '.env') -Key 'PYTHON_BIN' -Value '.venv/Scripts/python.exe'
    Write-Ok 'Python e JobSpy instalados no .venv.'
}

function Set-EnvValue {
    param(
        [Parameter(Mandatory = $true)][string]$Path,
        [Parameter(Mandatory = $true)][string]$Key,
        [Parameter(Mandatory = $true)][string]$Value
    )
    if (-not (Test-Path $Path)) {
        throw "Arquivo .env não encontrado em '$Path'."
    }

    $content = Get-Content -Path $Path -Raw
    $line = "$Key=`"$Value`""
    $pattern = "(?m)^\s*#?\s*$([regex]::Escape($Key))\s*=.*$"
    if ($content -match $pattern) {
        $content = [regex]::Replace($content, $pattern, $line, 1)
    }
    else {
        $content = $content.TrimEnd() + "`r`n$line`r`n"
    }
    $utf8NoBom = New-Object System.Text.UTF8Encoding($false)
    [System.IO.File]::WriteAllText($Path, $content, $utf8NoBom)
}

function Initialize-Repository {
    param([Parameter(Mandatory = $true)][string]$RepositoryRoot)

    if (-not (Test-Path (Join-Path $RepositoryRoot 'Cargo.toml'))) {
        if (Test-Path $RepositoryRoot) {
            $existing = Get-ChildItem -Path $RepositoryRoot -Force -ErrorAction SilentlyContinue
            if ($existing) {
                throw "A pasta '$RepositoryRoot' existe, mas não contém o Job Hunter RS. Escolha outro -InstallDir."
            }
        }
        else {
            New-Item -ItemType Directory -Path $RepositoryRoot -Force | Out-Null
        }

        Write-Step 'Baixando o repositório Job Hunter RS'
        & git clone $RepoUrl $RepositoryRoot
        if ($LASTEXITCODE -ne 0) {
            throw "Falha ao clonar $RepoUrl."
        }
    }
    else {
        Write-Ok "Repositório encontrado em $RepositoryRoot."
    }

    $envPath = Join-Path $RepositoryRoot '.env'
    if (-not (Test-Path $envPath)) {
        $examplePath = Join-Path $RepositoryRoot '.env.example'
        if (-not (Test-Path $examplePath)) {
            throw 'O arquivo .env.example não foi encontrado no repositório.'
        }
        Copy-Item $examplePath $envPath
        Write-Ok '.env criado a partir de .env.example.'
    }
    else {
        Write-Ok '.env existente preservado.'
    }
}

function Build-Application {
    if ($SkipBuild) {
        Write-Step 'Compilação ignorada por -SkipBuild'
        return
    }

    Write-Step 'Compilando o Job Hunter RS em modo release'
    Push-Location $script:RepoRoot
    try {
        & cargo build --release
        if ($LASTEXITCODE -ne 0) {
            throw 'cargo build --release falhou. Consulte a seção de solução de problemas do guia.'
        }

        if ($RunTests) {
            Write-Step 'Executando a suíte de testes'
            & cargo test
            if ($LASTEXITCODE -ne 0) {
                throw 'cargo test falhou.'
            }
        }
    }
    finally {
        Pop-Location
    }
    Write-Ok 'Executável criado em target\release\job-hunter.exe.'
}

try {
    Assert-Windows64
    $script:RepoRoot = Split-Path -Parent $PSScriptRoot

    if (-not (Test-Path (Join-Path $script:RepoRoot 'Cargo.toml'))) {
        $script:RepoRoot = [Environment]::ExpandEnvironmentVariables($InstallDir)
    }

    Write-Host 'Job Hunter RS - instalador para Windows' -ForegroundColor White
    Write-Host "Destino: $script:RepoRoot"

    if (-not $SkipSystemTools) {
        Assert-Administrator
        Install-BuildTools
        Install-Git
        Install-Rust
    }
    else {
        Write-Step 'Validação sem instalação de ferramentas do sistema'
        if (-not (Test-Command 'git')) { throw 'Git não encontrado. Remova -SkipSystemTools ou instale o Git.' }
        if (-not (Test-Command 'cargo')) { throw 'cargo não encontrado. Remova -SkipSystemTools ou instale o Rust.' }
        if (-not (Test-BuildTools)) { throw 'Microsoft C++ Build Tools não encontrado. Remova -SkipSystemTools ou instale o workload C++.' }
    }

    Initialize-Repository -RepositoryRoot $script:RepoRoot
    Install-Python
    Build-Application

    Write-Host "`nInstalação concluída." -ForegroundColor Green
    Write-Host 'Antes do primeiro uso, edite o arquivo .env e informe GEMINI_API_KEY.'
    Write-Host "Guia: $(Join-Path $script:RepoRoot 'docs\INSTALACAO_WINDOWS.md')"
    Write-Host "Executar: & '$(Join-Path $script:RepoRoot 'target\release\job-hunter.exe')' web --port 8787"
    Write-Host 'Depois abra http://localhost:8787 no navegador.'
}
catch {
    Write-Host "`nERRO: $($_.Exception.Message)" -ForegroundColor Red
    Write-Host 'Consulte docs\INSTALACAO_WINDOWS.md para instalação manual e diagnóstico.' -ForegroundColor Yellow
    if (-not $NoPause) {
        Read-Host 'Pressione Enter para sair' | Out-Null
    }
    exit 1
}

if (-not $NoPause) {
    Read-Host 'Pressione Enter para sair' | Out-Null
}
