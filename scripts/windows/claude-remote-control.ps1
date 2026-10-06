<#
.SYNOPSIS
Keeps "claude remote-control" running in a HoploDex checkout, and restarts it
to apply Claude Code updates.

.DESCRIPTION
For a Windows test machine with auto-logon (DEVELOPMENT.md, "Windows").
setup-user.ps1 -ClaudeRemoteControl copies this script to
%LOCALAPPDATA%\HoploDex and starts it, minimized, whenever the user signs
in. Claude then runs in the desktop session, where the app's window can
open for E2E and screenshots, and you reach it from claude.ai/code or the
Claude app. An SSH session couldn't host it: Windows' sshd ends every
process in a session when it disconnects, and the session has no desktop.

It starts "claude remote-control" in -Checkout and starts it again whenever
it stops. Claude Code downloads updates in the background, but a running
claude keeps its version until it restarts, so once a day, during
-UpdateHour, it restarts claude if a newer version is installed. Run with
-Restart (from SSH, say) to restart it now. Either way it stops claude with
Ctrl+C, as you would, and with Stop-Process only if that doesn't work, and
"claude remote-control" brings back the sessions the last one served.
Whatever a session was doing at that moment stops.

To stop it until the next sign-in, close its window.

.PARAMETER Checkout
The HoploDex checkout to serve sessions in.

.PARAMETER UpdateHour
The hour (0-23, local time) in which to restart for an update. -1 turns
that off. Defaults to 5.

.PARAMETER Restart
Restart the running claude now, picking up any installed update.
#>
#Requires -Version 5.1
[CmdletBinding(DefaultParameterSetName = 'Run')]
param(
    [Parameter(Mandatory = $true, ParameterSetName = 'Run')]
    [string]$Checkout,
    [Parameter(ParameterSetName = 'Run')]
    [ValidateRange(-1, 23)]
    [int]$UpdateHour = 5,
    [Parameter(Mandatory = $true, ParameterSetName = 'Restart')]
    [switch]$Restart
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$StateDir = Join-Path $env:LOCALAPPDATA 'HoploDex'
# Holds the running wrapper's process ID.
$PidFile = Join-Path $StateDir 'claude-remote-control.pid'
# -Restart creates it; the wrapper deletes it once it has restarted claude.
$RestartFile = Join-Path $StateDir 'claude-remote-control.restart'

function Write-Status([string]$Message) {
    Write-Host "$(Get-Date -Format 'yyyy-MM-dd HH:mm:ss') $Message"
}

# The running wrapper, if there is one.
function Get-Wrapper {
    $id = Get-Content $PidFile -ErrorAction SilentlyContinue
    if (-not $id) { return $null }
    $process = Get-Process -Id $id -ErrorAction SilentlyContinue
    if ($process -and $process.Id -ne $PID -and @('powershell', 'pwsh') -contains $process.ProcessName) {
        return $process
    }
    return $null
}

function Get-ClaudeVersion {
    $ErrorActionPreference = 'Continue'
    return "$(& claude --version 2>$null)".Trim()
}

if ($Restart) {
    if (-not (Get-Wrapper)) { throw 'claude-remote-control.ps1 is not running.' }
    New-Item -ItemType File -Force $RestartFile | Out-Null
    $deadline = (Get-Date).AddMinutes(2)
    while ((Test-Path $RestartFile) -and (Get-Date) -lt $deadline) { Start-Sleep -Seconds 2 }
    if (Test-Path $RestartFile) { throw 'The wrapper did not restart claude within 2 minutes.' }
    Write-Host 'claude remote-control restarted.'
    return
}

New-Item -ItemType Directory -Force $StateDir | Out-Null
if (Get-Wrapper) { throw 'claude-remote-control.ps1 is already running.' }
Set-Content $PidFile $PID
Remove-Item $RestartFile -ErrorAction SilentlyContinue
Set-Location $Checkout
$Host.UI.RawUI.WindowTitle = "claude remote-control: $Checkout"

Add-Type -TypeDefinition @'
using System;
using System.Runtime.InteropServices;

public static class HoploDexConsole {
    [DllImport("kernel32.dll", SetLastError = true)]
    public static extern bool GenerateConsoleCtrlEvent(uint ctrlEvent, uint processGroupId);

    [DllImport("kernel32.dll", SetLastError = true)]
    public static extern bool SetConsoleCtrlHandler(IntPtr handler, bool add);
}
'@

# Stops claude as Ctrl+C in this window would, but without stopping this
# script, then forcibly if it's still running after 30 seconds.
function Stop-Claude([System.Diagnostics.Process]$Claude) {
    # Ignore Ctrl+C here while claude, which shares this console, gets it.
    [HoploDexConsole]::SetConsoleCtrlHandler([IntPtr]::Zero, $true) | Out-Null
    try {
        foreach ($attempt in 1, 2) {
            [HoploDexConsole]::GenerateConsoleCtrlEvent(0, 0) | Out-Null
            if ($Claude.WaitForExit(15000)) { return }
        }
        Write-Status 'claude ignored Ctrl+C; stopping it.'
        Stop-Process -Id $Claude.Id -Force
        $Claude.WaitForExit()
    } finally {
        [HoploDexConsole]::SetConsoleCtrlHandler([IntPtr]::Zero, $false) | Out-Null
    }
}

while ($true) {
    $version = Get-ClaudeVersion
    Write-Status "Starting claude remote-control ($version) in $Checkout."
    $claudeExe = (Get-Command claude -CommandType Application | Select-Object -First 1).Source
    $claude = Start-Process $claudeExe -ArgumentList 'remote-control' -NoNewWindow -PassThru
    # Keep the handle, so the exit code is still there after it exits.
    $null = $claude.Handle
    $lastCheck = Get-Date
    $restarting = $false
    while (-not $claude.WaitForExit(5000)) {
        if (Test-Path $RestartFile) {
            Write-Status 'Restarting, as asked.'
            $restarting = $true
        } elseif ($UpdateHour -ge 0 -and (Get-Date).Hour -eq $UpdateHour -and
            ((Get-Date) - $lastCheck).TotalMinutes -ge 5) {
            $lastCheck = Get-Date
            $installed = Get-ClaudeVersion
            if ($installed -and $installed -ne $version) {
                Write-Status "Restarting for the update to $installed."
                $restarting = $true
            }
        }
        if ($restarting) {
            Stop-Claude $claude
            break
        }
    }
    if ($restarting) {
        Remove-Item $RestartFile -ErrorAction SilentlyContinue
    } else {
        Write-Status "claude stopped (exit code $($claude.ExitCode)); starting it again in 30 seconds."
        Start-Sleep -Seconds 30
    }
}
