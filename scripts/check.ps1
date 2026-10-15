<#
.SYNOPSIS
    Sprawdza całe repozytorium: formatowanie, lint, typy, testy i budowanie.

.DESCRIPTION
    Uruchamiany przez check.bat. Wykonuje kolejno wszystkie kroki (także po błędzie
    w którymś z nich) i na końcu wypisuje podsumowanie. Te same kontrole wykonuje CI.
    Kod wyjścia: 0, gdy wszystkie kroki przeszły; 1, gdy któryś zawiódł.

.PARAMETER Quick
    Pomija wolniejsze kroki: testy end-to-end (Playwright), Lighthouse, migracje, testy bazy i test serwera.
#>
[CmdletBinding()]
param(
    [switch] $Quick
)

Set-StrictMode -Version Latest
. (Join-Path $PSScriptRoot 'common.ps1')
Initialize-DevShell

# Uruchamia zbudowany serwer z bazą z Dockera i sprawdza /healthz oraz /readyz.
# Baza, której wcześniej nie było, zostaje po teście zatrzymana.
function Invoke-ServerSmokeTest {
    $wasRunning = [bool](docker @ComposeArgs ps --status running --quiet postgres)
    if (-not $wasRunning) {
        docker @ComposeArgs up -d --wait
        if ($LASTEXITCODE -ne 0) { return }
    }

    $ok = $false
    $server = $null
    $log = Join-Path ([IO.Path]::GetTempPath()) 'game-server-smoke.log'
    try {
        cargo build -p server --locked
        if ($LASTEXITCODE -eq 0) {
            # Osobny port, żeby test nie kolidował z serwerem uruchomionym przez run.bat.
            $addr = '127.0.0.1:18090'
            $vars = @{ DATABASE_URL = $DevDatabaseUrl; MIGRATION_DATABASE_URL = $DevDatabaseUrl; LISTEN_ADDR = $addr; LOG_FORMAT = 'text' }
            $saved = @{}
            foreach ($name in $vars.Keys) {
                $saved[$name] = [Environment]::GetEnvironmentVariable($name)
                [Environment]::SetEnvironmentVariable($name, $vars[$name])
            }
            try {
                & (Join-Path $RepoRoot 'target\debug\server.exe') migrate
                if ($LASTEXITCODE -eq 0) {
                    $server = Start-Process -FilePath (Join-Path $RepoRoot 'target\debug\server.exe') -PassThru `
                        -NoNewWindow -RedirectStandardOutput $log -RedirectStandardError "$log.err"
                }
            } finally {
                # Wartość $null usuwa zmienną, czyli przywraca stan sprzed testu.
                foreach ($name in $saved.Keys) { [Environment]::SetEnvironmentVariable($name, $saved[$name]) }
            }

            $health = if ($server) { Wait-HttpOk "http://$addr/healthz" 30 }
            $ready = if ($health) { Wait-HttpOk "http://$addr/readyz" 15 }
            Write-Host "  /healthz: $(if ($health) { $health } else { 'brak odpowiedzi 200' })"
            Write-Host "  /readyz:  $(if ($ready) { $ready } else { 'brak odpowiedzi 200' })"
            $ok = [bool]($health -and $ready)
        }
    } finally {
        if ($server) { Stop-Process -Id $server.Id -Force -ErrorAction SilentlyContinue }
        if (-not $wasRunning) { docker @ComposeArgs stop *> $null }
    }

    if ($server -and -not $ok) {
        Write-Host '  Logi serwera:' -ForegroundColor Yellow
        Get-Content $log, "$log.err" -ErrorAction SilentlyContinue | Select-Object -Last 20 | ForEach-Object { "    $_" }
    }
    $global:LASTEXITCODE = if ($ok) { 0 } else { 1 }
}

# Testy sqlx tworzą i usuwają wyłącznie własne, tymczasowe bazy. Baza deweloperska zostaje.
function Invoke-DatabaseChecks {
    if (-not (Test-Tools @('cargo-sqlx'))) { $global:LASTEXITCODE = 1; return }
    $wasRunning = [bool](docker @ComposeArgs ps --status running --quiet postgres)
    if (-not $wasRunning) {
        docker @ComposeArgs up -d --wait
        if ($LASTEXITCODE -ne 0) { return }
    }
    $vars = @{ DATABASE_URL = $DevDatabaseUrl; MIGRATION_DATABASE_URL = $DevDatabaseUrl; SQLX_OFFLINE = 'true' }
    $saved = @{}
    $exitCode = 1
    foreach ($name in $vars.Keys) {
        $saved[$name] = [Environment]::GetEnvironmentVariable($name)
        [Environment]::SetEnvironmentVariable($name, $vars[$name])
    }
    try {
        cargo run -q -p server --locked -- migrate
        if ($LASTEXITCODE -eq 0) {
            cargo test -p server --tests --locked -- --ignored
            if ($LASTEXITCODE -eq 0) {
                $env:SQLX_OFFLINE = 'false'
                cargo sqlx prepare --check --workspace -- --all-targets --locked
                $exitCode = $LASTEXITCODE
            }
        }
    } finally {
        foreach ($name in $saved.Keys) { [Environment]::SetEnvironmentVariable($name, $saved[$name]) }
        if (-not $wasRunning) { docker @ComposeArgs stop *> $null }
    }
    $global:LASTEXITCODE = $exitCode
}

$steps = @(
    @{ Name = 'Rust: formatowanie (cargo fmt)'; Run = { cargo fmt --all --check } }
    @{ Name = 'Rust: lint (clippy)'; Run = { cargo clippy --workspace --all-targets --locked -- -D warnings } }
    @{ Name = 'Treść: walidacja (content-compiler)'; Run = { cargo run -q -p content-compiler --locked -- check } }
    @{ Name = 'Rust: testy'; Run = { cargo test --workspace --locked } }
    @{ Name = 'Balans: postacie startowe (sim matrix)'; Run = { cargo run -q -p sim --locked -- matrix --n 500 } }
    @{ Name = 'AI: hierarchia botów i profili (sim ai, release)'; Run = { cargo run -q --release -p sim --locked -- ai --n 1200 } }
    @{ Name = 'Rust: silnik do WebAssembly'; Run = { cargo build -p engine-wasm --target wasm32-unknown-unknown --locked } }
    @{
        Name = 'Silnik: natywne = WASM (Node)'
        Needs = 'Rust: silnik do WebAssembly'
        Skip = { if (-not (Get-Command wasm-bindgen-test-runner -ErrorAction SilentlyContinue)) { 'brak wasm-bindgen-test-runner, zob. CONTRIBUTING.md' } }
        Run = { cargo test -p engine-wasm --target wasm32-unknown-unknown --locked }
    }
    @{ Name = 'Node: instalacja zależności'; Run = { pnpm install --frozen-lockfile } }
    @{ Name = 'Web: format i lint (Biome)'; Run = { pnpm exec biome check } }
    @{ Name = 'Web: zasoby WASM i treść'; Run = { pnpm --filter web prepare:assets } }
    @{ Name = 'Web: typy (tsc)'; Run = { pnpm --recursive --if-present typecheck } }
    @{ Name = 'Web: testy (Vitest)'; Run = { pnpm --filter web test } }
    @{ Name = 'Web: budowanie klienta'; Run = { pnpm --filter web build } }
    @{ Name = 'Proto: lint i format (buf)'; Run = { pnpm exec buf lint; if ($LASTEXITCODE -eq 0) { pnpm exec buf format --diff --exit-code } } }
    @{
        Name = 'Baza: migracje, ograniczenia i sqlx prepare'
        Slow = $true
        Skip = { if (-not (Get-Command docker -ErrorAction SilentlyContinue)) { 'brak Dockera' } elseif (-not (Test-DockerRunning)) { 'Docker nie działa' } }
        Run = { Invoke-DatabaseChecks }
    }
    @{
        Name = 'E2E: Playwright'
        Slow = $true
        Skip = { if (-not (Get-Command docker -ErrorAction SilentlyContinue)) { 'brak Dockera' } elseif (-not (Test-DockerRunning)) { 'Docker nie działa' } }
        # Testy e2e działają na zbudowanym kliencie; bez udanego budowania wynik byłby nieaktualny.
        Needs = 'Web: budowanie klienta'
        Run = { pnpm --filter e2e exec playwright install chromium; if ($LASTEXITCODE -eq 0) { pnpm --filter e2e test } }
    }
    @{
        Name = 'Web: Lighthouse mobile (>=95)'
        Slow = $true
        Needs = 'Web: budowanie klienta'
        Run = { pnpm audit:web }
    }
    @{
        Name = 'Serwer: /healthz i /readyz z bazą'
        Slow = $true
        Skip = { if (-not (Get-Command docker -ErrorAction SilentlyContinue)) { 'brak Dockera' } elseif (-not (Test-DockerRunning)) { 'Docker nie działa' } }
        Run = { Invoke-ServerSmokeTest }
    }
)

$results = New-Object System.Collections.Generic.List[object]

function Add-Result([string] $Name, [string] $Status, [double] $Seconds, [string] $Note = '') {
    $results.Add([pscustomobject]@{ Name = $Name; Status = $Status; Seconds = $Seconds; Note = $Note })
}

if (-not (Test-Tools @('cargo', 'node', 'pnpm', 'wasm-bindgen'))) {
    Wait-IfLaunchedFromExplorer
    exit 1
}

Write-Host "Sprawdzanie repozytorium: $RepoRoot" -ForegroundColor Cyan
Write-Host "$(cargo --version) | node $(node --version) | pnpm $(pnpm --version)"

$total = [Diagnostics.Stopwatch]::StartNew()
foreach ($step in $steps) {
    if ($Quick -and $step.ContainsKey('Slow')) {
        Add-Result $step.Name 'POMINIĘTO' 0 'tryb -Quick'
        continue
    }
    if ($step.ContainsKey('Needs')) {
        $needed = $results | Where-Object { $_.Name -eq $step.Needs }
        if (-not $needed -or $needed.Status -ne 'OK') {
            Add-Result $step.Name 'POMINIĘTO' 0 "wymaga kroku: $($step.Needs)"
            continue
        }
    }
    if ($step.ContainsKey('Skip')) {
        $reason = & $step.Skip
        if ($reason) {
            Add-Result $step.Name 'POMINIĘTO' 0 $reason
            continue
        }
    }

    Write-Host ''
    Write-Host "==> $($step.Name)" -ForegroundColor Cyan
    $watch = [Diagnostics.Stopwatch]::StartNew()
    $global:LASTEXITCODE = 0
    $ok = $true
    try {
        & $step.Run
        $ok = ($LASTEXITCODE -eq 0)
    } catch {
        Write-Host $_ -ForegroundColor Red
        $ok = $false
    }
    $watch.Stop()
    Add-Result $step.Name ($(if ($ok) { 'OK' } else { 'BŁĄD' })) $watch.Elapsed.TotalSeconds
}
$total.Stop()

Write-Host ''
Write-Host 'Podsumowanie' -ForegroundColor Cyan
foreach ($r in $results) {
    $color = switch ($r.Status) { 'OK' { 'Green' } 'BŁĄD' { 'Red' } default { 'Yellow' } }
    $time = if ($r.Seconds -gt 0) { '{0,7:N1} s' -f $r.Seconds } else { '         ' }
    $note = if ($r.Note) { "  ($($r.Note))" } else { '' }
    Write-Host ('  {0,-10} {1} {2}{3}' -f $r.Status, $time, $r.Name, $note) -ForegroundColor $color
}

$failed = @($results | Where-Object { $_.Status -eq 'BŁĄD' })
Write-Host ''
if ($failed.Count -eq 0) {
    Write-Host ('Wszystko w porządku ({0:N0} s).' -f $total.Elapsed.TotalSeconds) -ForegroundColor Green
} else {
    Write-Host "Kroki z błędami: $($failed.Count). Szczegóły są wyżej, w wynikach poszczególnych kroków." -ForegroundColor Red
}

Wait-IfLaunchedFromExplorer
exit $(if ($failed.Count -eq 0) { 0 } else { 1 })
