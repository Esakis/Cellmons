<#
.SYNOPSIS
    Uruchamia grę lokalnie: bazę danych (Docker), serwer gry i klienta, potem otwiera przeglądarkę.

.DESCRIPTION
    Uruchamiany przez run.bat. Serwer i klient działają w osobnych oknach; zamknięcie okna
    zatrzymuje daną usługę. Baza działa w Dockerze do wywołania: run.bat -Stop.
    Jeśli serwer albo klient już działa, skrypt go nie uruchamia drugi raz.

.PARAMETER Stop
    Zatrzymuje bazę danych. Dane zostają w woluminie Dockera.

.PARAMETER NoBrowser
    Nie otwiera przeglądarki.
#>
[CmdletBinding()]
param(
    [switch] $Stop,
    [switch] $NoBrowser
)

Set-StrictMode -Version Latest
. (Join-Path $PSScriptRoot 'common.ps1')
Initialize-DevShell

$clientUrl = 'http://localhost:5173'

function Exit-WithError([string] $Message) {
    Write-Host $Message -ForegroundColor Red
    Wait-IfLaunchedFromExplorer
    exit 1
}

# Otwiera nowe okno PowerShell z tytułem i poleceniem; okno zostaje otwarte po błędzie.
function Start-ServiceWindow([string] $Title, [string] $Command) {
    $script = "`$Host.UI.RawUI.WindowTitle = '$Title'; $Command"
    Start-Process powershell -WorkingDirectory $RepoRoot -ArgumentList @('-NoExit', '-NoProfile', '-Command', $script) | Out-Null
}

if ($Stop) {
    if (-not (Test-Tools @('docker'))) { Exit-WithError 'Nie można zatrzymać bazy bez Dockera.' }
    docker @ComposeArgs stop
    if ($LASTEXITCODE -ne 0) { Exit-WithError 'Nie udało się zatrzymać bazy danych.' }
    Write-Host 'Baza danych zatrzymana. Dane zostały w woluminie Dockera.' -ForegroundColor Green
    Wait-IfLaunchedFromExplorer
    exit 0
}

if (-not (Test-Tools @('cargo', 'node', 'pnpm', 'docker'))) { Exit-WithError 'Uzupełnij brakujące programy i uruchom ponownie.' }

if (-not (Test-Path (Join-Path $RepoRoot '.env'))) {
    Copy-Item (Join-Path $RepoRoot '.env.example') (Join-Path $RepoRoot '.env')
    Write-Host 'Utworzono plik .env na podstawie .env.example.' -ForegroundColor Yellow
}

Write-Host '==> Docker' -ForegroundColor Cyan
if (-not (Test-DockerRunning)) {
    $dockerDesktop = Join-Path $env:ProgramFiles 'Docker\Docker\Docker Desktop.exe'
    if (-not (Test-Path $dockerDesktop)) { Exit-WithError 'Docker nie działa. Uruchom Docker Desktop i spróbuj ponownie.' }
    Write-Host 'Uruchamiam Docker Desktop (może to potrwać do 2 minut)...'
    Start-Process $dockerDesktop
    $deadline = (Get-Date).AddMinutes(2)
    while (-not (Test-DockerRunning)) {
        if ((Get-Date) -gt $deadline) { Exit-WithError 'Docker nie uruchomił się w ciągu 2 minut.' }
        Start-Sleep -Seconds 2
    }
}

Write-Host '==> Baza danych (PostgreSQL w Dockerze)' -ForegroundColor Cyan
docker @ComposeArgs up -d --wait
if ($LASTEXITCODE -ne 0) { Exit-WithError 'Baza danych nie wystartowała. Szczegóły są wyżej.' }

$listenAddr = Get-DotEnvValue 'LISTEN_ADDR' '0.0.0.0:8080'
$serverUrl = 'http://' + $listenAddr.Replace('0.0.0.0', '127.0.0.1')

Write-Host '==> Serwer gry' -ForegroundColor Cyan
if (Wait-HttpOk "$serverUrl/healthz" 1) {
    Write-Host "Serwer już działa ($serverUrl)."
} else {
    # Budowanie w tym oknie, żeby błędy kompilacji były od razu widoczne.
    cargo build -p server
    if ($LASTEXITCODE -ne 0) { Exit-WithError 'Budowanie serwera nie powiodło się. Szczegóły są wyżej.' }
    # Działa kopia programu: Windows blokuje plik uruchomionego programu, a check.bat buduje
    # serwer w target\debug także wtedy, gdy ten serwer działa.
    $runDir = Join-Path $RepoRoot 'target\run'
    try {
        New-Item -ItemType Directory -Force -Path $runDir -ErrorAction Stop | Out-Null
        Copy-Item -Force -Path (Join-Path $RepoRoot 'target\debug\server.exe') -Destination $runDir -ErrorAction Stop
    } catch {
        Exit-WithError "Nie można skopiować serwera do $runDir ($($_.Exception.Message)). Zamknij okno 'Spellgambit: serwer gry' i spróbuj ponownie."
    }
    Start-ServiceWindow 'Spellgambit: serwer gry' '& .\target\run\server.exe'
    if (-not (Wait-HttpOk "$serverUrl/healthz" 30)) {
        Exit-WithError "Serwer nie odpowiada na $serverUrl/healthz. Sprawdź okno 'Spellgambit: serwer gry'."
    }
}

Write-Host '==> Klient (Vite)' -ForegroundColor Cyan
if (Wait-HttpOk $clientUrl 1) {
    Write-Host "Klient już działa ($clientUrl)."
} else {
    pnpm install --frozen-lockfile
    if ($LASTEXITCODE -ne 0) { Exit-WithError 'Instalacja zależności klienta nie powiodła się.' }
    Start-ServiceWindow 'Spellgambit: klient' 'pnpm --filter web dev'
    if (-not (Wait-HttpOk $clientUrl 60)) {
        Exit-WithError "Klient nie odpowiada na $clientUrl. Sprawdź okno 'Spellgambit: klient'."
    }
}

$ready = Wait-HttpOk "$serverUrl/readyz" 10
Write-Host ''
Write-Host 'Gra działa lokalnie.' -ForegroundColor Green
Write-Host "  Klient:      $clientUrl"
Write-Host "  Serwer gry:  $serverUrl/healthz"
Write-Host "  Baza danych: 127.0.0.1:5433 ($(if ($ready) { 'działa' } else { 'serwer nie może się połączyć' }))"
Write-Host ''
Write-Host 'Zatrzymanie: zamknij okna serwera i klienta, a bazę zatrzymaj poleceniem: run.bat -Stop'

if (-not $NoBrowser) { Start-Process $clientUrl }
Wait-IfLaunchedFromExplorer
