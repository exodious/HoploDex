<#
.SYNOPSIS
Runs a command in the signed-in user's desktop session, follows its output,
and exits with its status: the Windows counterpart of scripts/tart-vm.sh gui.

.DESCRIPTION
For a Windows test machine with auto-logon, driven over SSH (DEVELOPMENT.md,
"Windows"). A command started over Windows' OpenSSH has no desktop, so the
app's window can't open for E2E, screenshots or the native UI checks, and
sshd ends it when the connection drops. So this runs the command through a
scheduled task instead: "HoploDex gui", registered for this user (it needs
no administrator) with an interactive logon, not elevated, which Task
Scheduler starts in the user's signed-in desktop session, outside the SSH
connection. Builds, cargo nextest and Vitest don't need it: run them over
SSH directly.

The command is PowerShell, run in a new powershell.exe in -WorkingDirectory,
in a minimized console window. Its output and errors go to a log in
%LOCALAPPDATA%\HoploDex\gui\<run>\, which this follows until the command's
exit status is written there, then prints it and exits with it: the last
native command's exit code, 1 if the command threw, or what it passes to
exit. Interrupting this, or losing the connection, leaves the command
running; its log stays in that folder (named on stderr at the start).

The user must be signed in at the console (auto-logon does it) and the
session not disconnected: a disconnected Remote Desktop session stops
drawing, so screenshots come out black.

.PARAMETER Command
The PowerShell command line to run, such as 'npm run test:e2e'.

.PARAMETER WorkingDirectory
Where to run it. Defaults to the checkout this script is in.

.PARAMETER RunDirectory
Internal: the run folder that the scheduled task passes to this script in
the desktop session.
#>
#Requires -Version 5.1
[CmdletBinding(DefaultParameterSetName = 'Run')]
param(
    [Parameter(Mandatory = $true, Position = 0, ParameterSetName = 'Run')]
    [string]$Command,
    [Parameter(ParameterSetName = 'Run')]
    [string]$WorkingDirectory,
    [Parameter(Mandatory = $true, ParameterSetName = 'InSession')]
    [string]$RunDirectory
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
# Over SSH, progress records reach stderr as CLIXML.
$ProgressPreference = 'SilentlyContinue'

$TaskName = 'HoploDex gui'

function Stop-WithError([string]$Message) {
    [Console]::Error.WriteLine("gui: $Message")
    exit 1
}

# In the desktop session, started by the task: run the command with its output
# in the log, then write its status. The status file appears whole (a rename),
# so the follower never reads it half-written.
if ($PSCmdlet.ParameterSetName -eq 'InSession') {
    $log = Join-Path $RunDirectory 'log'
    $code = 1
    try {
        Set-Content -LiteralPath (Join-Path $RunDirectory 'pid') -Value $PID -Encoding Ascii
        $start = New-Object System.Diagnostics.ProcessStartInfo 'cmd.exe'
        # cmd's redirection, so the log holds the command's bytes as it wrote
        # them; PowerShell's own (5.1) would re-encode them as UTF-16.
        $start.Arguments = '/d /s /c "powershell.exe -NoProfile -ExecutionPolicy Bypass -File "{0}" >> "{1}" 2>&1"' -f `
            (Join-Path $RunDirectory 'command.ps1'), $log
        $start.WorkingDirectory = Get-Content -LiteralPath (Join-Path $RunDirectory 'cwd') -Encoding UTF8
        $start.UseShellExecute = $false
        $process = [System.Diagnostics.Process]::Start($start)
        $process.WaitForExit()
        $code = $process.ExitCode
    } catch {
        Add-Content -LiteralPath $log -Value "gui: $_" -Encoding UTF8
    }
    Set-Content -LiteralPath (Join-Path $RunDirectory 'status.tmp') -Value $code -Encoding Ascii
    Move-Item -LiteralPath (Join-Path $RunDirectory 'status.tmp') -Destination (Join-Path $RunDirectory 'status')
    exit $code
}

# Over SSH: check there is a desktop to run in.
$me = $env:USERNAME
# quser complains on stderr when there is none, which 5.1 turns into a
# terminating error under Stop.
$session = @(& { $ErrorActionPreference = 'Continue'; quser $me 2>$null } | Select-Object -Skip 1 | Where-Object { $_ -match '\bActive\b' })
if ($session.Count -eq 0) {
    Stop-WithError "$me has no active desktop session; sign it in at the console (auto-logon does at startup) and don't disconnect it"
}
# Here, not as the parameter's default: 5.1 has no $PSScriptRoot there under -File.
if (-not $WorkingDirectory) { $WorkingDirectory = Split-Path -Parent (Split-Path -Parent $PSScriptRoot) }
if (-not (Test-Path -LiteralPath $WorkingDirectory -PathType Container)) {
    Stop-WithError "no such directory: $WorkingDirectory"
}

$runs = Join-Path $env:LOCALAPPDATA 'HoploDex\gui'
$run = Join-Path $runs ('{0:yyyyMMdd-HHmmss}-{1}' -f (Get-Date), $PID)
$null = New-Item -ItemType Directory -Path $run
$utf8 = New-Object System.Text.UTF8Encoding $true # PowerShell 5.1 reads a BOM-less script as ANSI
# UTF-8 output, as node and cargo write theirs, not the console's code page;
# exit $LASTEXITCODE, so -File passes the last native command's status on.
$script = "[Console]::OutputEncoding = [System.Text.Encoding]::UTF8`r`n$Command`r`nexit `$LASTEXITCODE`r`n"
[System.IO.File]::WriteAllText((Join-Path $run 'command.ps1'), $script, $utf8)
[System.IO.File]::WriteAllText((Join-Path $run 'cwd'), (Resolve-Path -LiteralPath $WorkingDirectory).Path, $utf8)
$logPath = Join-Path $run 'log'
[System.IO.File]::WriteAllBytes($logPath, @())

# The task, created or updated on every run so it always points at this
# script. Through Task Scheduler's COM API, which a standard user may use
# for their own tasks (the ScheduledTasks cmdlets go through CIM and are
# refused), and which passes the run folder to the task as $(Arg0), so
# concurrent runs don't race on the task's definition.
$TASK_ACTION_EXEC = 0
$TASK_CREATE_OR_UPDATE = 6
$TASK_LOGON_INTERACTIVE_TOKEN = 3
$TASK_RUNLEVEL_LUA = 0
$TASK_INSTANCES_PARALLEL = 0
$identity = [System.Security.Principal.WindowsIdentity]::GetCurrent().Name
try {
    $scheduler = New-Object -ComObject Schedule.Service
    $scheduler.Connect()
    $definition = $scheduler.NewTask(0)
    $definition.RegistrationInfo.Description = "Runs a command in $me's desktop session for $PSCommandPath."
    $definition.Principal.UserId = $identity
    $definition.Principal.LogonType = $TASK_LOGON_INTERACTIVE_TOKEN
    $definition.Principal.RunLevel = $TASK_RUNLEVEL_LUA
    $settings = $definition.Settings
    $settings.ExecutionTimeLimit = 'PT0S' # no limit; the default is 72 hours
    $settings.DisallowStartIfOnBatteries = $false
    $settings.StopIfGoingOnBatteries = $false
    $settings.MultipleInstances = $TASK_INSTANCES_PARALLEL
    $settings.Priority = 4 # normal; a task's default (7) is below normal, which slows builds
    $action = $definition.Actions.Create($TASK_ACTION_EXEC)
    $action.Path = Join-Path $PSHOME 'powershell.exe'
    $action.Arguments = "-NoProfile -ExecutionPolicy Bypass -WindowStyle Minimized -File `"$PSCommandPath`" -RunDirectory `"`$(Arg0)`""
    $task = $scheduler.GetFolder('\').RegisterTaskDefinition($TaskName, $definition, $TASK_CREATE_OR_UPDATE, $identity, $null, $TASK_LOGON_INTERACTIVE_TOKEN)
    $null = $task.Run($run)
} catch {
    Stop-WithError "couldn't start the scheduled task '$TaskName': $_"
}
[Console]::Error.WriteLine("gui: running in $me's desktop session; log: $logPath")

# Follow the log, as it's written, until the status appears.
$out = [Console]::OpenStandardOutput()
$reader = [System.IO.File]::Open($logPath, 'Open', 'Read', 'ReadWrite, Delete')
$buffer = New-Object byte[] 65536
$statusPath = Join-Path $run 'status'
$pidPath = Join-Path $run 'pid'
$started = Get-Date
function Copy-NewOutput {
    while (($n = $reader.Read($buffer, 0, $buffer.Length)) -gt 0) { $out.Write($buffer, 0, $n) }
    $out.Flush()
}
while (-not (Test-Path -LiteralPath $statusPath)) {
    Copy-NewOutput
    Start-Sleep -Milliseconds 500
    if (Test-Path -LiteralPath $pidPath) {
        $runner = Get-Content -LiteralPath $pidPath -ErrorAction SilentlyContinue
        if ($runner -and -not (Get-Process -Id $runner -ErrorAction SilentlyContinue) -and -not (Test-Path -LiteralPath $statusPath)) {
            Copy-NewOutput
            Stop-WithError "the run ended without an exit status (signed out, or ended from Task Manager?); log: $logPath"
        }
    } elseif (((Get-Date) - $started).TotalSeconds -gt 60) {
        Stop-WithError "the scheduled task '$TaskName' didn't start within a minute (last result $('0x{0:X8}' -f $task.LastTaskResult)); log: $logPath"
    }
}
Copy-NewOutput
$reader.Close()
$status = [int](Get-Content -LiteralPath $statusPath)
Remove-Item -LiteralPath $run -Recurse -Force -ErrorAction SilentlyContinue
exit $status
