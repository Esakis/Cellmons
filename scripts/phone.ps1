# Local Android testing with persistent accounts. All generated data stays in target/phone-test.
[CmdletBinding()]
param(
    [string] $Address,
    [switch] $Rebuild,
    [switch] $Stop
)
$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest
. (Join-Path $PSScriptRoot 'common.ps1')
Initialize-DevShell
$phoneDirectory = Join-Path $RepoRoot 'target\phone-test'
$runtimePath = Join-Path $phoneDirectory 'runtime.json'
New-Item -ItemType Directory -Force -Path $phoneDirectory | Out-Null

function Read-Runtime {
    if (Test-Path -LiteralPath $runtimePath) {
        return Get-Content -LiteralPath $runtimePath -Raw | ConvertFrom-Json
    }
    return [pscustomobject]@{ backend = $null; gateway = $null }
}
function Stop-OwnedProcess($Record) {
    if (-not $Record) { return }
    $process = Get-Process -Id $Record.id -ErrorAction SilentlyContinue
    if ($process -and $process.StartTime.ToUniversalTime().ToString('o') -eq $Record.started -and $process.Path -eq $Record.path) {
        Stop-Process -Id $process.Id
        [void]$process.WaitForExit(10000)
    }
}
function Process-Record($Process, [string] $Executable) {
    # Immediately after launch, Process.Path can transiently report ntdll.dll on Windows.
    return @{ id = $Process.Id; started = $Process.StartTime.ToUniversalTime().ToString('o'); path = [IO.Path]::GetFullPath($Executable) }
}
function Save-Runtime($Runtime) {
    $Runtime | ConvertTo-Json -Depth 4 | Set-Content -Encoding UTF8 -LiteralPath $runtimePath
}
function Invoke-PhoneNative([string] $File, [string[]] $Arguments) {
    # Windows PowerShell 5 wraps native stderr as ErrorRecord when output is redirected.
    # Cargo, Docker and Gradle write normal progress there; use the actual exit code.
    $ErrorActionPreference = 'Continue'
    & $File @Arguments
    if ($LASTEXITCODE -ne 0) { throw "$File failed (exit $LASTEXITCODE)." }
}
function Test-PhoneDocker {
    $ErrorActionPreference = 'Continue'
    docker info *> $null
    return $LASTEXITCODE -eq 0
}

$runtime = Read-Runtime
if ($Stop) {
    Stop-OwnedProcess $runtime.gateway
    Stop-OwnedProcess $runtime.backend
    Save-Runtime ([pscustomobject]@{ backend = $null; gateway = $null })
    Write-Host 'Test telefonu zatrzymany. Konta i postep zostaly zachowane.'
    exit 0
}

if (-not $Address) {
    $route = Get-NetRoute -DestinationPrefix '0.0.0.0/0' |
        Where-Object { $_.NextHop -ne '0.0.0.0' } |
        Sort-Object RouteMetric | Select-Object -First 1
    if (-not $route) { throw 'No LAN connection. Pass -Address with the private IPv4 address of this computer.' }
    $Address = (Get-NetIPAddress -InterfaceIndex $route.InterfaceIndex -AddressFamily IPv4 |
        Where-Object { $_.IPAddress -notlike '169.254.*' } | Select-Object -First 1).IPAddress
}
if (-not (Get-NetIPAddress -AddressFamily IPv4 | Where-Object IPAddress -eq $Address)) {
    throw 'The selected address does not belong to this computer.'
}
if (-not (Test-Tools @('docker', 'cargo', 'node', 'pnpm'))) { throw 'Missing required tools.' }
if (-not (Test-Path -LiteralPath (Join-Path $RepoRoot '.env'))) {
    Copy-Item -LiteralPath (Join-Path $RepoRoot '.env.example') -Destination (Join-Path $RepoRoot '.env')
}
$database = Get-DotEnvValue 'DATABASE_URL' $DevDatabaseUrl
$dbUri = [Uri]$database
if ($dbUri.Host -notin @('127.0.0.1', 'localhost', '[::1]')) { throw 'Phone testing requires the local development database.' }
if (-not (Test-PhoneDocker)) {
    Start-Process (Join-Path $env:ProgramFiles 'Docker\Docker\Docker Desktop.exe') -WindowStyle Hidden
    $deadline = (Get-Date).AddMinutes(3)
    while (-not (Test-PhoneDocker)) {
        if ((Get-Date) -gt $deadline) { throw 'Docker did not start.' }
        Start-Sleep -Seconds 2
    }
}
Invoke-PhoneNative 'docker' ($ComposeArgs + @('up', '-d', '--wait'))
$env:LISTEN_ADDR = '127.0.0.1:8090'
$env:PUBLIC_ORIGIN = 'http://localhost:5173'
$env:MAIL_MODE = 'file'
$env:MAIL_OUTBOX = 'target/mail'
$env:LOBBY_BOTS_ENABLED = 'true'
$env:DATABASE_URL = $database

if ($Rebuild) {
    Stop-OwnedProcess $runtime.backend
    $runtime.backend = $null
    Save-Runtime $runtime
    if (Wait-HttpOk 'http://127.0.0.1:8090/readyz' 1) {
        throw 'Port 8090 is owned by another server. Stop that server before rebuilding the phone test.'
    }
}
if (-not (Wait-HttpOk 'http://127.0.0.1:8090/readyz' 1)) {
    Invoke-PhoneNative 'cargo' @('build', '-p', 'server', '--locked')
    Invoke-PhoneNative (Join-Path $RepoRoot 'target\debug\server.exe') @('migrate')
    Invoke-PhoneNative (Join-Path $RepoRoot 'target\debug\server.exe') @('seed-bots')
    $serverFile = Join-Path $phoneDirectory 'server.exe'
    Copy-Item -LiteralPath (Join-Path $RepoRoot 'target\debug\server.exe') -Destination $serverFile -Force
    $serverProcess = Start-Process $serverFile -WorkingDirectory $RepoRoot -WindowStyle Hidden -PassThru `
        -RedirectStandardOutput (Join-Path $phoneDirectory 'server.stdout.log') `
        -RedirectStandardError (Join-Path $phoneDirectory 'server.stderr.log')
    $runtime.backend = Process-Record $serverProcess $serverFile
    Save-Runtime $runtime
    if (-not (Wait-HttpOk 'http://127.0.0.1:8090/readyz' 45)) { throw 'Game server failed; inspect target/phone-test/server.stderr.log.' }
}
Invoke-PhoneNative 'node' @('scripts/phone-gateway.mjs', 'prepare', $Address)
$connection = Get-Content -Encoding UTF8 (Join-Path $phoneDirectory 'connection.json') -Raw | ConvertFrom-Json
$buildPath = Join-Path $phoneDirectory 'build.json'
$previous = if (Test-Path $buildPath) { Get-Content $buildPath -Raw | ConvertFrom-Json } else { $null }
$needsBuild = $Rebuild -or -not (Test-Path (Join-Path $phoneDirectory 'Spellgambit-test.apk')) -or
    -not $previous -or $previous.apiOrigin -ne $connection.apiOrigin -or
    $previous.certificateSha256 -ne $connection.certificateSha256
if ($needsBuild) {
    $javaCandidates = @($env:JAVA_HOME, (Join-Path $env:ProgramFiles 'Android\Android Studio\jbr'))
    $javaCandidates += @(Get-ChildItem (Join-Path $RepoRoot 'target\android-tools') -Directory -Filter 'jdk-*' -ErrorAction SilentlyContinue | ForEach-Object FullName)
    $selectedJava = $javaCandidates | Where-Object {
        $_ -and (Test-Path (Join-Path $_ 'release')) -and
        (@(Get-Content (Join-Path $_ 'release') | Select-String '^JAVA_VERSION="(21|2[2-9])\.').Count -gt 0)
    } | Select-Object -First 1
    if (-not $selectedJava) {
        throw 'JDK 21 or newer is required. Set JAVA_HOME to its installation directory.'
    }
    $env:JAVA_HOME = $selectedJava
    if (-not $env:ANDROID_HOME) { $env:ANDROID_HOME = Join-Path $env:LOCALAPPDATA 'Android\Sdk' }
    $env:ANDROID_APPLICATION_ID = $connection.packageId
    $env:ANDROID_VERSION = '1.0.5'
    $env:VITE_API_ORIGIN = $connection.apiOrigin
    $env:E2E_TLS_CERT = Join-Path $phoneDirectory 'cert.pem'
    $env:WEB_BUNDLE_VERSION = '0'
    $env:WEB_BUNDLE_URL = ''
    $env:WEB_BUNDLE_PUBLIC_KEY = ''
    Invoke-PhoneNative 'pnpm' @('--filter', 'mobile', 'sync')
    Invoke-PhoneNative '.\mobile\android\gradlew.bat' @('-p', 'mobile/android', 'assembleDebug')
    Copy-Item -LiteralPath (Join-Path $RepoRoot 'mobile\android\app\build\outputs\apk\debug\app-debug.apk') `
        -Destination (Join-Path $phoneDirectory 'Spellgambit-test.apk') -Force
    $connection | ConvertTo-Json | Set-Content -Encoding UTF8 $buildPath
}

Stop-OwnedProcess $runtime.gateway
$nodeFile = (Get-Command node).Source
$gatewayFile = Join-Path $PSScriptRoot 'phone-gateway.mjs'
$gateway = Start-Process $nodeFile -ArgumentList @("`"$gatewayFile`"", 'serve') `
    -WorkingDirectory $RepoRoot -WindowStyle Hidden -PassThru `
    -RedirectStandardOutput (Join-Path $phoneDirectory 'gateway.stdout.log') `
    -RedirectStandardError (Join-Path $phoneDirectory 'gateway.stderr.log')
$runtime.gateway = Process-Record $gateway $nodeFile
Save-Runtime $runtime
if (-not (Wait-HttpOk 'http://127.0.0.1:18080/' 10)) { throw 'Phone gateway failed; inspect target/phone-test/gateway.stderr.log.' }

# A narrow rule for this Node executable, these two ports and private local networks.
$ruleName = 'Spellgambit-Phone-Test'
if ((Get-NetFirewallProfile -Name Private).Enabled -and -not (Get-NetFirewallRule -Name $ruleName -ErrorAction SilentlyContinue)) {
    try {
        New-NetFirewallRule -Name $ruleName -DisplayName 'Spellgambit phone test (private LAN)' `
            -Direction Inbound -Action Allow -Protocol TCP -LocalPort 18080,18443 `
            -RemoteAddress LocalSubnet -Profile Private -Program $nodeFile | Out-Null
    } catch {
        Write-Warning 'Firewall rule could not be added. If the phone cannot connect, run phone.bat once as administrator on your private home network.'
    }
}
Invoke-PhoneNative 'node' @('scripts/phone-gateway.mjs', 'account')
Invoke-PhoneNative 'node' @('scripts/phone-gateway.mjs', 'check')
Write-Host ''
Write-Host "Na telefonie otworz: $($connection.downloadOrigin)" -ForegroundColor Green
Write-Host 'Login i haslo: target\phone-test\konto-testowe.json'
Write-Host 'Gra: Konto -> zaloguj sie -> Lobby -> Arena -> wybierz gracza -> wyzwanie.'
Write-Host 'Zatrzymanie: phone.bat -Stop. Po zmianach kodu: phone.bat -Rebuild.'
