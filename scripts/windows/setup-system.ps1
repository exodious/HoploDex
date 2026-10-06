<#
.SYNOPSIS
Installs HoploDex's machine-wide Windows build prerequisites.

.DESCRIPTION
The machine-wide half of a Windows development setup (DEVELOPMENT.md,
"Windows"). Run it once per computer, from an elevated PowerShell, then run
setup-user.ps1 as each account that builds:

  powershell -ExecutionPolicy Bypass -File setup-system.ps1

It installs, skipping whatever is already there:
  - Visual Studio Build Tools 2026 with the "Desktop development with C++"
    workload and its recommended parts (the MSVC compiler and linker, and a
    Windows SDK). A Visual Studio that already has the C++ tools counts.
  - Git, machine-wide.
  - Node.js 24 LTS, pinned in winget so an upgrade stays on 24.
  - The WebView2 runtime, if Windows doesn't already have it.
  - vcpkg in -VcpkgRoot, with OpenSSL 3 (openssl:x64-windows-static-md) for
    SQLCipher, and the machine-wide OPENSSL_DIR and OPENSSL_STATIC=1 the
    build reads. OpenSSL 1.1 isn't GPLv3-compatible, so it checks for 3.
  - Win32 long paths, which deep node_modules and target paths can need.
  - With -GitHubCli, the GitHub CLI; with -OpenSsh, the OpenSSH server.

It can be run again: it adds only what's missing. x64 Windows only.

.PARAMETER VcpkgRoot
Where to clone vcpkg. Defaults to C:\vcpkg.

.PARAMETER GitHubCli
Also install the GitHub CLI (gh).

.PARAMETER OpenSsh
Also install and start the OpenSSH server, to run commands from another
computer.
#>
#Requires -Version 5.1
#Requires -RunAsAdministrator
[CmdletBinding()]
param(
    [string]$VcpkgRoot = 'C:\vcpkg',
    [switch]$GitHubCli,
    [switch]$OpenSsh
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$NodeMajor = 24
$OpenSslTriplet = 'x64-windows-static-md'
$VcWorkload = 'Microsoft.VisualStudio.Workload.VCTools'
$VcTools = 'Microsoft.VisualStudio.Component.VC.Tools.x86.x64'
# winget's "reboot needed to finish installation" (0x8A150109).
$WingetRebootNeeded = -1978334967
$script:RebootNeeded = $false

function Write-Step([string]$Message) {
    Write-Host "==> $Message" -ForegroundColor Cyan
}

# Runs a program, shows its output, and returns its exit code, throwing if
# that isn't one of OkCodes.
function Invoke-Checked {
    param([string]$File, [string[]]$Arguments, [int[]]$OkCodes = @(0))
    # Progress on stderr (git, winget) is not an error.
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

function Install-WingetPackage {
    param([string]$Id, [string[]]$Extra = @())
    $arguments = @('install', '--id', $Id, '--exact', '--source', 'winget',
        '--accept-source-agreements', '--accept-package-agreements',
        '--disable-interactivity') + $Extra
    $code = Invoke-Checked winget $arguments -OkCodes @(0, $WingetRebootNeeded)
    if ($code -eq $WingetRebootNeeded) { $script:RebootNeeded = $true }
    Update-SessionPath
}

function Get-VsWhere {
    return Join-Path ${env:ProgramFiles(x86)} 'Microsoft Visual Studio\Installer\vswhere.exe'
}

# The newest Visual Studio or Build Tools that has the x64 C++ tools.
function Get-VcInstallation {
    $vswhere = Get-VsWhere
    if (-not (Test-Path $vswhere)) { return $null }
    $path = & $vswhere -latest -products * -requires $VcTools -property installationPath
    if ($path) { return $path }
    return $null
}

function Test-WindowsSdk {
    $lib = Join-Path ${env:ProgramFiles(x86)} 'Windows Kits\10\Lib\*\um\x64\kernel32.Lib'
    return [bool](Get-ChildItem $lib -ErrorAction SilentlyContinue)
}

if ($env:PROCESSOR_ARCHITECTURE -ne 'AMD64') {
    throw "This setup is for x64 Windows; this computer is $env:PROCESSOR_ARCHITECTURE."
}
if (-not (Test-Command winget)) {
    throw 'winget is missing. Install "App Installer" from the Microsoft Store, then run this again.'
}

Write-Step 'C++ build tools'
if ((Get-VcInstallation) -and (Test-WindowsSdk)) {
    Write-Host "Found the C++ tools in $(Get-VcInstallation)."
} else {
    $vswhere = Get-VsWhere
    $buildTools = $null
    if (Test-Path $vswhere) {
        $buildTools = & $vswhere -latest -products Microsoft.VisualStudio.Product.BuildTools -property installationPath
    }
    if ($buildTools) {
        # winget won't add a workload to an existing install, so ask the
        # Visual Studio Installer to.
        Write-Host "Adding the C++ workload to $buildTools."
        $installer = Join-Path ${env:ProgramFiles(x86)} 'Microsoft Visual Studio\Installer\setup.exe'
        $process = Start-Process $installer -Wait -PassThru -ArgumentList @(
            'modify', '--installPath', "`"$buildTools`"", '--add', $VcWorkload,
            '--includeRecommended', '--passive', '--norestart')
        if ($process.ExitCode -eq 3010) {
            $script:RebootNeeded = $true
        } elseif ($process.ExitCode -ne 0) {
            throw "The Visual Studio Installer failed with exit code $($process.ExitCode)."
        }
    } else {
        Install-WingetPackage 'Microsoft.VisualStudio.BuildTools' @(
            '--override', "--wait --passive --norestart --add $VcWorkload --includeRecommended")
    }
    if (-not (Get-VcInstallation)) { throw 'The C++ tools are still missing after installing them.' }
    if (-not (Test-WindowsSdk)) { throw 'The Windows SDK is still missing after installing the C++ tools.' }
}

Write-Step 'Git'
if (Test-Command git) {
    Write-Host "Found $(git --version)."
} else {
    Install-WingetPackage 'Git.Git' @('--scope', 'machine')
}

Write-Step "Node.js $NodeMajor LTS"
if (Test-Command node) {
    $found = (node --version).TrimStart('v')
    if (([version]$found).Major -ne $NodeMajor) {
        throw "Node.js $found is installed; HoploDex needs $NodeMajor. Uninstall it, then run this again."
    }
    Write-Host "Found Node.js $found."
} else {
    # OpenJS.NodeJS.LTS follows whichever release is LTS, so pick the newest 24.x.
    $versions = @(winget show --id OpenJS.NodeJS.LTS --exact --versions --source winget `
        --accept-source-agreements --disable-interactivity |
        Where-Object { $_ -match "^\s*$NodeMajor\.\d+\.\d+\s*$" } |
        ForEach-Object { [version]$_.Trim() } |
        Sort-Object -Descending)
    if (-not $versions) { throw "winget lists no Node.js $NodeMajor.x release." }
    Install-WingetPackage 'OpenJS.NodeJS.LTS' @('--version', $versions[0].ToString())
}
# Keep "winget upgrade --all" from moving to the next LTS.
Invoke-Checked winget @('pin', 'add', '--id', 'OpenJS.NodeJS.LTS', '--exact',
    '--version', "$NodeMajor.*", '--force', '--accept-source-agreements',
    '--disable-interactivity') | Out-Null

Write-Step 'WebView2 runtime'
$webView2 = '{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}'
$webView2Keys = @(
    "HKLM:\SOFTWARE\WOW6432Node\Microsoft\EdgeUpdate\Clients\$webView2",
    "HKLM:\SOFTWARE\Microsoft\EdgeUpdate\Clients\$webView2")
$webView2Version = $webView2Keys |
    ForEach-Object { (Get-ItemProperty $_ -ErrorAction SilentlyContinue).pv } |
    Where-Object { $_ -and $_ -ne '0.0.0.0' } |
    Select-Object -First 1
if ($webView2Version) {
    Write-Host "Found WebView2 $webView2Version."
} else {
    Install-WingetPackage 'Microsoft.EdgeWebView2Runtime' @('--scope', 'machine')
}

Write-Step "vcpkg and OpenSSL ($VcpkgRoot)"
if (-not (Test-Path (Join-Path $VcpkgRoot '.git'))) {
    Invoke-Checked git @('clone', 'https://github.com/microsoft/vcpkg', $VcpkgRoot) | Out-Null
}
$vcpkg = Join-Path $VcpkgRoot 'vcpkg.exe'
if (-not (Test-Path $vcpkg)) {
    Invoke-Checked (Join-Path $VcpkgRoot 'bootstrap-vcpkg.bat') @('-disableMetrics') | Out-Null
}
Invoke-Checked $vcpkg @('install', "openssl:$OpenSslTriplet") | Out-Null
$opensslDir = Join-Path $VcpkgRoot "installed\$OpenSslTriplet"
$header = Get-Content (Join-Path $opensslDir 'include\openssl\opensslv.h') -Raw
if ($header -notmatch 'OPENSSL_VERSION_MAJOR\s+(\d+)' -or [int]$Matches[1] -lt 3) {
    throw "vcpkg installed an OpenSSL older than 3 in $opensslDir; HoploDex's license needs OpenSSL 3."
}
[Environment]::SetEnvironmentVariable('OPENSSL_DIR', $opensslDir, 'Machine')
[Environment]::SetEnvironmentVariable('OPENSSL_STATIC', '1', 'Machine')
Write-Host "OPENSSL_DIR=$opensslDir, OPENSSL_STATIC=1 (machine-wide)."

Write-Step 'Long paths'
Set-ItemProperty 'HKLM:\SYSTEM\CurrentControlSet\Control\FileSystem' -Name LongPathsEnabled -Value 1 -Type DWord
Invoke-Checked git @('config', '--system', 'core.longpaths', 'true') | Out-Null

if ($GitHubCli) {
    Write-Step 'GitHub CLI'
    if (Test-Command gh) {
        Write-Host "Found $((gh --version)[0])."
    } else {
        Install-WingetPackage 'GitHub.cli' @('--scope', 'machine')
    }
}

if ($OpenSsh) {
    Write-Step 'OpenSSH server'
    $capability = Get-WindowsCapability -Online -Name 'OpenSSH.Server*' | Select-Object -First 1
    if ($capability.State -ne 'Installed') {
        Add-WindowsCapability -Online -Name $capability.Name | Out-Null
    }
    Set-Service sshd -StartupType Automatic
    Start-Service sshd
}

Write-Host ''
Write-Host 'Machine-wide setup is done. Next, as each account that builds HoploDex,' -ForegroundColor Green
Write-Host 'open a new PowerShell (not elevated) and run setup-user.ps1.' -ForegroundColor Green
if ($script:RebootNeeded) {
    Write-Warning 'An installer asked for a restart. Restart Windows before building.'
}
