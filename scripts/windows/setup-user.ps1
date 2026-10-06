<#
.SYNOPSIS
Installs HoploDex's per-user Windows development tools.

.DESCRIPTION
The per-user half of a Windows development setup (DEVELOPMENT.md,
"Windows"). Run setup-system.ps1 first, then run this as the account that
will build, in a new PowerShell that is not elevated:

  powershell -ExecutionPolicy Bypass -File setup-user.ps1

Everything here lands in that account's profile, so an elevated shell
signed in as another administrator would set up the wrong account.

It installs, skipping or updating whatever is already there:
  - rustup (%USERPROFILE%\.cargo, %USERPROFILE%\.rustup) with the stable
    toolchain, rustfmt and clippy.
  - npm 12 (%APPDATA%\npm; Node's own npm.cmd hands off to it).
  - cargo-nextest and cargo-deny, built from source with --locked.
  - With -ClaudeCode, Claude Code, from its own installer.
  - With -ClaudeRemoteControl <checkout>, for a test machine with
    auto-logon: Claude Code, and claude-remote-control.ps1 (copied to
    %LOCALAPPDATA%\HoploDex) started minimized from the Startup folder at
    each sign-in, serving Remote Control sessions in that checkout from the
    desktop session. It needs claude-remote-control.ps1 next to this script.

It checks first that setup-system.ps1 has run: the C++ tools, Git, Node.js
24 and OPENSSL_DIR.

.PARAMETER ClaudeCode
Also install Claude Code (https://claude.ai/install.ps1).

.PARAMETER ClaudeRemoteControl
A HoploDex checkout to serve Claude Code Remote Control sessions in, from
the desktop session, starting at each sign-in. Implies -ClaudeCode.
#>
#Requires -Version 5.1
[CmdletBinding()]
param(
    [switch]$ClaudeCode,
    [string]$ClaudeRemoteControl
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$NodeMajor = 24
$NpmMajor = 12
# rust-version in src-tauri/Cargo.toml.
$RustMinimum = [version]'1.97.0'
$VcTools = 'Microsoft.VisualStudio.Component.VC.Tools.x86.x64'

function Write-Step([string]$Message) {
    Write-Host "==> $Message" -ForegroundColor Cyan
}

# Runs a program, shows its output, and returns its exit code, throwing if
# that isn't one of OkCodes.
function Invoke-Checked {
    param([string]$File, [string[]]$Arguments, [int[]]$OkCodes = @(0))
    # Progress on stderr (cargo, rustup, npm) is not an error.
    $ErrorActionPreference = 'Continue'
    & $File @Arguments | Out-Host
    if ($OkCodes -notcontains $LASTEXITCODE) {
        throw "$File $($Arguments -join ' ') failed with exit code $LASTEXITCODE"
    }
    return $LASTEXITCODE
}

# Installers change PATH in the registry, not in this session.
function Update-SessionPath {
    $env:Path = [Environment]::GetEnvironmentVariable('Path', 'Machine') + ';' +
        [Environment]::GetEnvironmentVariable('Path', 'User')
}

function Test-Command([string]$Name) {
    return [bool](Get-Command $Name -ErrorAction SilentlyContinue)
}

function Get-Major([string]$Version) {
    return ([version]($Version.Trim().TrimStart('v') -replace '[-+].*$', '')).Major
}

Write-Host "Setting up $env:USERDOMAIN\$env:USERNAME ($env:USERPROFILE)."

if ($ClaudeRemoteControl) {
    $ClaudeCode = $true
    if (-not (Test-Path (Join-Path $ClaudeRemoteControl '.git'))) {
        throw "$ClaudeRemoteControl isn't a git checkout. Clone HoploDex there first."
    }
    $ClaudeRemoteControl = (Resolve-Path $ClaudeRemoteControl).Path
    $wrapperSource = Join-Path $PSScriptRoot 'claude-remote-control.ps1'
    if (-not (Test-Path $wrapperSource)) { throw "$wrapperSource is missing; copy it next to this script." }
}

Write-Step 'Checking the machine-wide setup'
Update-SessionPath
$missing = @()
$vswhere = Join-Path ${env:ProgramFiles(x86)} 'Microsoft Visual Studio\Installer\vswhere.exe'
if (-not ((Test-Path $vswhere) -and (& $vswhere -latest -products * -requires $VcTools -property installationPath))) {
    $missing += 'the C++ build tools'
}
if (-not (Test-Command git)) { $missing += 'Git' }
if (-not (Test-Command node)) {
    $missing += "Node.js $NodeMajor"
} elseif ((Get-Major (node --version)) -ne $NodeMajor) {
    $missing += "Node.js $NodeMajor (found $(node --version))"
}
$opensslDir = [Environment]::GetEnvironmentVariable('OPENSSL_DIR', 'Machine')
if (-not $opensslDir) { $opensslDir = [Environment]::GetEnvironmentVariable('OPENSSL_DIR', 'User') }
if (-not $opensslDir -or -not (Test-Path (Join-Path $opensslDir 'include\openssl\opensslv.h'))) {
    $missing += 'OpenSSL (OPENSSL_DIR)'
}
if ($missing) {
    throw "Missing $($missing -join ', '). Run setup-system.ps1 from an elevated PowerShell first."
}
$env:OPENSSL_DIR = $opensslDir
$env:OPENSSL_STATIC = '1'

Write-Step 'Rust'
$cargoBin = Join-Path $env:USERPROFILE '.cargo\bin'
if (-not (Test-Path (Join-Path $cargoBin 'rustup.exe'))) {
    Invoke-Checked winget @('install', '--id', 'Rustlang.Rustup', '--exact', '--source', 'winget',
        '--accept-source-agreements', '--accept-package-agreements',
        '--disable-interactivity') | Out-Null
    Update-SessionPath
}
if (($env:Path -split ';') -notcontains $cargoBin) { $env:Path = "$cargoBin;$env:Path" }
Invoke-Checked rustup @('toolchain', 'install', 'stable', '--profile', 'minimal',
    '--component', 'rustfmt', '--component', 'clippy') | Out-Null
Invoke-Checked rustup @('default', 'stable') | Out-Null
$rustc = rustc --version
if ($rustc -notmatch '^rustc (\d+\.\d+\.\d+)' -or [version]$Matches[1] -lt $RustMinimum) {
    throw "$rustc is older than $RustMinimum."
}
$rustHost = "$((rustc -vV) -match '^host:')"
if ($rustHost -notmatch 'x86_64-pc-windows-msvc') {
    throw "The default toolchain isn't x86_64-pc-windows-msvc. Run: rustup set default-host x86_64-pc-windows-msvc"
}
Write-Host "Using $rustc."

Write-Step "npm $NpmMajor"
if ((Get-Major (npm --version)) -lt $NpmMajor) {
    Invoke-Checked npm @('install', '--global', "npm@$NpmMajor") | Out-Null
}
$npmVersion = npm --version
if ((Get-Major $npmVersion) -lt $NpmMajor) {
    throw "npm is still $npmVersion after installing npm $NpmMajor; check PATH for another npm."
}
Write-Host "Using npm $npmVersion."

Write-Step 'cargo-nextest and cargo-deny'
# Already-installed versions are skipped; newer releases replace them.
Invoke-Checked cargo @('install', '--locked', 'cargo-nextest', 'cargo-deny') | Out-Null

if ($ClaudeCode) {
    Write-Step 'Claude Code'
    if (Test-Command claude) {
        Write-Host "Found Claude Code $(claude --version)."
    } else {
        $installer = Join-Path $env:TEMP 'claude-install.ps1'
        Invoke-WebRequest https://claude.ai/install.ps1 -OutFile $installer -UseBasicParsing
        & $installer
        Remove-Item $installer
        Update-SessionPath
        $claudeBin = Join-Path $env:USERPROFILE '.local\bin'
        if (($env:Path -split ';') -notcontains $claudeBin) { $env:Path = "$claudeBin;$env:Path" }
    }
}

if ($ClaudeRemoteControl) {
    Write-Step 'Claude Code Remote Control at sign-in'
    # A copy outside the checkout, so switching branches there can't remove it.
    $stateDir = Join-Path $env:LOCALAPPDATA 'HoploDex'
    New-Item -ItemType Directory -Force $stateDir | Out-Null
    $wrapper = Join-Path $stateDir 'claude-remote-control.ps1'
    Copy-Item $wrapperSource $wrapper -Force
    $startup = [Environment]::GetFolderPath('Startup')
    $shortcut = (New-Object -ComObject WScript.Shell).CreateShortcut(
        (Join-Path $startup 'HoploDex Claude remote control.lnk'))
    $shortcut.TargetPath = Join-Path $env:SystemRoot 'System32\WindowsPowerShell\v1.0\powershell.exe'
    $shortcut.Arguments = "-NoProfile -ExecutionPolicy Bypass -File `"$wrapper`" -Checkout `"$ClaudeRemoteControl`""
    $shortcut.WorkingDirectory = $ClaudeRemoteControl
    # Minimized.
    $shortcut.WindowStyle = 7
    $shortcut.Save()
    Write-Host "At each sign-in, claude remote-control starts in $ClaudeRemoteControl."
}

Write-Host ''
Write-Host 'Done. Open a new terminal so PATH and OPENSSL_DIR apply, then in the' -ForegroundColor Green
Write-Host 'checkout run "npm install" and the build and test commands in DEVELOPMENT.md.' -ForegroundColor Green
if ($ClaudeRemoteControl) {
    Write-Host ''
    Write-Host 'Before the next sign-in, answer Claude Code''s one-time questions, which the' -ForegroundColor Green
    Write-Host 'minimized window would otherwise wait on. In a new terminal:' -ForegroundColor Green
    Write-Host "  cd `"$ClaudeRemoteControl`""
    Write-Host '  claude                 # sign in with /login, then /exit'
    Write-Host '  claude remote-control  # trust the folder, enable Remote Control, then Ctrl+C'
    Write-Host 'Then restart Windows (or sign out and in) to start it.' -ForegroundColor Green
}
