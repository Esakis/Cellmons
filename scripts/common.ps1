# Wspólne funkcje skryptów check.ps1 i run.ps1 (wczytywane przez dot-sourcing).

$RepoRoot = Split-Path -Parent $PSScriptRoot

# Adres bazy z deploy/docker-compose.dev.yml (ten sam co w .env.example).
$DevDatabaseUrl = 'postgres://game:game@127.0.0.1:5433/game'
$ComposeArgs = @('compose', '-f', (Join-Path $RepoRoot 'deploy\docker-compose.dev.yml'))

# Przygotowuje sesję: katalog repozytorium, Cargo w PATH, polskie znaki w konsoli.
function Initialize-DevShell {
    # Terminale otwarte przed instalacją Rusta nie mają jeszcze Cargo w PATH.
    $cargoBin = Join-Path $env:USERPROFILE '.cargo\bin'
    if ((Test-Path $cargoBin) -and (($env:PATH -split ';') -notcontains $cargoBin)) {
        $env:PATH = "$cargoBin;$env:PATH"
    }
    # Cargo, pnpm i Vitest wypisują tekst w UTF-8.
    try { [Console]::OutputEncoding = [Text.Encoding]::UTF8 } catch { }
    # Pasek postępu Invoke-WebRequest spowalnia zapytania i zaśmieca wynik.
    $global:ProgressPreference = 'SilentlyContinue'
    Set-Location $RepoRoot
}

# Sprawdza, czy potrzebne programy są dostępne; dla brakujących wypisuje, jak je zainstalować.
function Test-Tools([string[]] $Names) {
    $hints = @{
        cargo  = 'Rust: https://rustup.rs (szczegóły w CONTRIBUTING.md)'
        pnpm   = 'pnpm: polecenie "corepack enable pnpm" (Node.js 24 LTS z https://nodejs.org)'
        node   = 'Node.js 24 LTS: https://nodejs.org'
        docker = 'Docker Desktop: https://www.docker.com/products/docker-desktop'
    }
    $missing = @($Names | Where-Object { -not (Get-Command $_ -ErrorAction SilentlyContinue) })
    foreach ($name in $missing) {
        Write-Host "Brak programu '$name'. Zainstaluj: $($hints[$name])" -ForegroundColor Red
    }
    return $missing.Count -eq 0
}

function Test-DockerRunning {
    docker info *> $null
    return $LASTEXITCODE -eq 0
}

# Czeka, aż adres odpowie kodem 200. Zwraca treść odpowiedzi albo $null po upływie limitu.
function Wait-HttpOk([string] $Url, [int] $TimeoutSeconds) {
    $deadline = (Get-Date).AddSeconds($TimeoutSeconds)
    do {
        try {
            $response = Invoke-WebRequest -UseBasicParsing -TimeoutSec 3 -Uri $Url
            if ($response.StatusCode -eq 200) { return $response.Content }
        } catch { }
        Start-Sleep -Milliseconds 500
    } while ((Get-Date) -lt $deadline)
    return $null
}

# Odczytuje wartość z pliku .env w katalogu głównym repozytorium.
function Get-DotEnvValue([string] $Name, [string] $Default) {
    $file = Join-Path $RepoRoot '.env'
    if (Test-Path $file) {
        foreach ($line in Get-Content -Encoding UTF8 $file) {
            if ($line -match "^\s*$Name\s*=\s*(.*?)\s*$" -and $Matches[1]) {
                return $Matches[1].Trim('"')
            }
        }
    }
    return $Default
}

# Dwuklik w Eksploratorze uruchamia: cmd.exe /c ""ścieżka.bat" " (ze spacją przed ostatnim
# cudzysłowem). Wtedy okno zamknęłoby się od razu po zakończeniu, więc czekamy na Enter.
function Wait-IfLaunchedFromExplorer {
    try {
        $self = Get-CimInstance Win32_Process -Filter "ProcessId=$PID" -ErrorAction Stop
        $parent = Get-CimInstance Win32_Process -Filter "ProcessId=$($self.ParentProcessId)" -ErrorAction Stop
        if ($parent.Name -eq 'cmd.exe' -and $parent.CommandLine -match ' "\s*$') {
            Write-Host ''
            [void](Read-Host 'Naciśnij Enter, aby zamknąć okno')
        }
    } catch { }
}
