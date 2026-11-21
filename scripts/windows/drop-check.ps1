# The drop check on Windows (code scanning alert `paths/cwe-73` on
# `add_document_from_path`, #207), in two parts, both without the `e2e`
# feature, so drops arrive as a shipped build gets them, and both driven with
# real input:
#
# 1. src-tauri\examples\drop_check.rs: the app's drop recorder and refusal on
#    a page of the check's own. It drags a file from another process's window
#    (examples\drag_source.ps1) onto the main window, which must be
#    attachable, and links out of the page and a second web view, which must
#    not be, and must not reach the recorder as a drop at all.
# 2. src-tauri\examples\app_drop_check.rs: the app itself, as a release
#    builds it (`cargo build --release --features custom-protocol`, after
#    `npm run build`), or the one -App names (an installed HoploDex.exe). It
#    opens a database of one firearm, and a photo and a PDF are dragged from
#    File Explorer (examples\explorer_source.ps1) onto that firearm's page;
#    both must be attached.
#
#   powershell -ExecutionPolicy Bypass -File scripts\windows\drop-check.ps1 [-App EXE] [EXAMPLE FLAGS...]
#
# Run it in the signed-in desktop session (over SSH, through
# scripts\windows\gui.ps1), at 100% scaling, as a user who doesn't use
# HoploDex: an app built without the `e2e` feature keeps its settings in the
# user's own %APPDATA%\io.github.exodious.HoploDex, which the Windows known
# folders don't let a test move. So part 2 refuses to run if that folder is
# there, unless an earlier run of this check left it (its `.drop-check` mark)
# with no machine.json in it, and deletes it afterwards. Its database and files are in a fresh scratch
# folder; the app's caches and web view data go to the user's
# %LOCALAPPDATA%\io.github.exodious.HoploDex as usual. The logs go to
# e2e\screenshots-out\drop-check\. Exits 0 if both parts passed, else the
# first failure's code (1 no window, database or way to the page, 3 usage,
# 4 a check failed, 5 the settings folder is someone's).
param(
    [string]$App,
    [Parameter(ValueFromRemainingArguments)][string[]]$Rest = @()
)
$ErrorActionPreference = 'Stop'
$Root = (Resolve-Path "$PSScriptRoot\..\..").Path
$Out = "$Root\e2e\screenshots-out\drop-check"
New-Item -ItemType Directory -Force $Out | Out-Null
$Config = "$env:APPDATA\io.github.exodious.HoploDex"
$Mark = "$Config\.drop-check"
# Whether the settings folder may be someone's: there and not this check's,
# or holding a machine.json (a run that was stopped leaves the mark and its
# machine.json, so one there now may be the user's since).
function Test-SomeonesConfig {
    (Test-Path $Config) -and (-not (Test-Path $Mark) -or (Test-Path "$Config\machine.json"))
}
function Stop-Someones {
    [Console]::Error.WriteLine("drop-check: $Config exists: HoploDex's settings for this user, which the app check would use. Run it as a user who doesn't use HoploDex.")
    exit 5
}
# Asked before the builds, and again just before the folder is made.
if (Test-SomeonesConfig) { Stop-Someones }

Push-Location "$Root\src-tauri"
cmd /c "cargo build --profile e2e --example drop_check --example app_drop_check 2>&1"
if ($LASTEXITCODE -ne 0) { exit 1 }
if (-not $App) {
    Push-Location $Root
    cmd /c "npm run build 2>&1"
    if ($LASTEXITCODE -ne 0) { exit 1 }
    Pop-Location
    cmd /c "cargo build --release --features custom-protocol 2>&1"
    if ($LASTEXITCODE -ne 0) { exit 1 }
    $App = "$Root\src-tauri\target\release\hoplodex.exe"
}
Pop-Location

$ps = @('powershell', '-NoProfile', '-ExecutionPolicy', 'Bypass', '-File')
$inputFlags = @()
foreach ($a in $ps + "$Root\src-tauri\examples\real_input.ps1") { $inputFlags += @('--input', $a) }

# Runs one part's example and logs its stderr; returns its exit code.
function Invoke-Part([string]$Exe, [string[]]$Arguments, [string]$Log) {
    # Start-Process joins the arguments with spaces, unquoted.
    $quoted = $Arguments | ForEach-Object { '"' + (($_ -replace '(\\*)"', '$1$1\"') -replace '(\\+)$', '$1$1') + '"' }
    $p = Start-Process -FilePath $Exe -PassThru -NoNewWindow -RedirectStandardError "$Log.stderr" -ArgumentList $quoted
    # Windows PowerShell 5.1 reads a null ExitCode unless the process's handle
    # was taken while it ran.
    $null = $p.Handle
    $p.WaitForExit()
    $code = $p.ExitCode
    Get-Content "$Log.stderr" | Set-Content $Log
    Remove-Item "$Log.stderr"
    Add-Content $Log "exit $code"
    Get-Content $Log | Where-Object { $_ -match 'CHECK|^exit' } | ForEach-Object { [Console]::Out.WriteLine($_) }
    return $code
}

$S = (New-Item -ItemType Directory -Force "$((Get-Item $env:TEMP).FullName)\drop-check-$([guid]::NewGuid().ToString('N').Substring(0, 6))").FullName
$codes = @()
$made = $false
try {
    $sourceFlags = @()
    foreach ($a in $ps + "$Root\src-tauri\examples\drag_source.ps1") { $sourceFlags += @('--source', $a) }
    $codes += Invoke-Part "$Root\src-tauri\target\e2e\examples\drop_check.exe" `
        (@('--scratch', "$S\scratch", '--out', $Out) + $inputFlags + $sourceFlags + $Rest) "$Out\drop-windows.log"

    if (Test-SomeonesConfig) { Stop-Someones }
    # Without -Force: it fails if the folder appeared since that check.
    if (-not (Test-Path $Mark)) { New-Item -ItemType Directory $Config | Out-Null }
    Set-Content $Mark 'made by scripts\windows\drop-check.ps1, which deletes this folder'
    $made = $true
    $sourceFlags = @()
    foreach ($a in $ps + "$Root\src-tauri\examples\explorer_source.ps1") { $sourceFlags += @('--source', $a) }
    $codes += Invoke-Part "$Root\src-tauri\target\e2e\examples\app_drop_check.exe" `
        (@('--app', $App, '--config', $Config, '--scratch', "$S\app", '--out', $Out) + $inputFlags + $sourceFlags) "$Out\app-drop-windows.log"
} finally {
    if ($made) { Remove-Item -Recurse -Force $Config -ErrorAction SilentlyContinue }
    Remove-Item -Recurse -Force $S -ErrorAction SilentlyContinue
}
$failed = @($codes | Where-Object { $_ -ne 0 })
if ($failed.Count -gt 0) { exit $failed[0] }
exit 0
