# The drop check on Windows (code scanning alert `paths/cwe-73` on
# `add_document_from_path`): builds src-tauri\examples\drop_check.rs without
# the `e2e` feature, so drops arrive as a shipped build gets them, and runs it
# with real input. It drags a file from another process's window
# (examples\drag_source.ps1) onto the main window, which must be attachable,
# and links out of the page and a second web view, which must not be.
#
#   powershell -ExecutionPolicy Bypass -File scripts\windows\drop-check.ps1 [EXAMPLE FLAGS...]
#
# Run it in the signed-in desktop session (over SSH, through
# scripts\windows\gui.ps1), at 100% scaling. It runs in a fresh scratch
# folder and never reaches a real database. The log goes to
# e2e\screenshots-out\drop-check\. Exits with the example's code (0 passed,
# 1 no window, 3 usage, 4 a check failed).
param([Parameter(ValueFromRemainingArguments)][string[]]$Rest = @())
$ErrorActionPreference = 'Stop'
$Root = (Resolve-Path "$PSScriptRoot\..\..").Path
$Out = "$Root\e2e\screenshots-out\drop-check"
New-Item -ItemType Directory -Force $Out | Out-Null

Push-Location "$Root\src-tauri"
cmd /c "cargo build --profile e2e --example drop_check 2>&1"
if ($LASTEXITCODE -ne 0) { exit 1 }
Pop-Location

$S = (New-Item -ItemType Directory -Force "$((Get-Item $env:TEMP).FullName)\drop-check-$([guid]::NewGuid().ToString('N').Substring(0, 6))").FullName
$Log = "$Out\drop-windows.log"
$ps = @('powershell', '-NoProfile', '-ExecutionPolicy', 'Bypass', '-File')
$flags = @()
foreach ($a in $ps + "$Root\src-tauri\examples\real_input.ps1") { $flags += @('--input', $a) }
foreach ($a in $ps + "$Root\src-tauri\examples\drag_source.ps1") { $flags += @('--source', $a) }
$exe = "$Root\src-tauri\target\e2e\examples\drop_check.exe"
$p = Start-Process -FilePath $exe -PassThru -NoNewWindow -RedirectStandardError "$Out\drop-windows.stderr.txt" `
    -ArgumentList (@('--scratch', "$S\scratch", '--out', $Out) + $flags + $Rest)
# Windows PowerShell 5.1 reads a null ExitCode unless the process's handle was
# taken while it ran.
$null = $p.Handle
$p.WaitForExit()
$code = $p.ExitCode
Get-Content "$Out\drop-windows.stderr.txt" | Set-Content $Log
Remove-Item "$Out\drop-windows.stderr.txt"
Add-Content $Log "exit $code"
Get-Content $Log | Where-Object { $_ -match 'CHECK|^exit' }
Remove-Item -Recurse -Force $S -ErrorAction SilentlyContinue
exit $code
