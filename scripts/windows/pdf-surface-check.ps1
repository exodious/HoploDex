# The PDF surface check on Windows (specs/007-document-preview/research.md
# §23): builds src-tauri\examples\pdf_surface_check.rs and runs it with real
# input. It shows the surface a hostile PDF, a truncated and a bit-flipped one
# and a 10 MB one, probes what a frame can reach (Edge's own viewer frame
# through the DevTools protocol), clicks, scrolls and presses keys, and fails
# if anything reached the network, a command or the disk. It prints the 10 MB
# PDF's time to first paint.
#
#   powershell -ExecutionPolicy Bypass -File scripts\windows\pdf-surface-check.ps1 [shows]
#
# Run it in the signed-in desktop session (the auto-logon session of
# DEVELOPMENT.md's Windows setup; an SSH session has no desktop to post input
# to or take a screenshot of), at 100% scaling. The app's folders come from the
# E2E variables (an `e2e` build) and are a fresh scratch folder's, and the
# surface's WebView2 user data folder is its own under it, never the app's.
# The log and the screenshot go to e2e\screenshots-out\pdf-surface\. Exits with
# the example's code (0 passed, 1 no window, 2 no load, 3 usage, 4 a check
# failed).
#
# Here the surface's proxy is a listener the check reads, in place of the app's
# tripwire (which only counts): `judge_proxied` in the example says what must and
# may reach it.
param([string]$Mode = 'check', [Parameter(ValueFromRemainingArguments)][string[]]$Rest = @())
$ErrorActionPreference = 'Stop'
$Root = (Resolve-Path "$PSScriptRoot\..\..").Path
$Out = "$Root\e2e\screenshots-out\pdf-surface"
New-Item -ItemType Directory -Force $Out | Out-Null

Push-Location "$Root\src-tauri"
cmd /c "cargo build --example pdf_surface_check --features e2e 2>&1"
if ($LASTEXITCODE -ne 0) { exit 1 }
Pop-Location

# The long form of the temp path: SendKeys reads the ~ of a short (8.3) path
# as Enter.
$S = (New-Item -ItemType Directory -Force "$((Get-Item $env:TEMP).FullName)\surface-$([guid]::NewGuid().ToString('N').Substring(0, 6))").FullName
New-Item -ItemType Directory -Force "$S\config", "$S\cache", "$S\documents" | Out-Null
$Log = "$Out\$Mode-windows.log"
$env:HOPLODEX_E2E_CONFIG_HOME = "$S\config"
$env:HOPLODEX_E2E_CACHE_HOME = "$S\cache"
$env:HOPLODEX_E2E_DOCUMENTS = "$S\documents"

$flags = @()
if ($Mode -eq 'check') {
    $input_ = "$Root\src-tauri\examples\pdf_surface_input.ps1"
    # Where the document could be copied to: the user's AppData (the temp
    # folder and WebView2's folders are in it) and the run's scratch folder.
    $flags = @('--input', 'powershell', '--input', '-NoProfile', '--input', '-ExecutionPolicy',
        '--input', 'Bypass', '--input', '-File', '--input', $input_,
        '--scan', $env:LOCALAPPDATA, '--scan', $env:APPDATA, '--scan', $env:TEMP, '--scan', $S,
        '--skip', "$Root\src-tauri\target", '--skip', "$Root\node_modules", '--skip', $Out)
}
$exe = "$Root\src-tauri\target\debug\examples\pdf_surface_check.exe"
$p = Start-Process -FilePath $exe -PassThru -NoNewWindow -RedirectStandardError "$Out\$Mode-windows.stderr.txt" `
    -ArgumentList (@($Mode, '--scratch', "$S\scratch", '--screenshot', "$Out\$Mode-windows.png") + $flags + $Rest)
# Windows PowerShell 5.1 reads a null ExitCode unless the process's handle was
# taken while it ran.
$null = $p.Handle
$p.WaitForExit()
$code = $p.ExitCode
Get-Content "$Out\$Mode-windows.stderr.txt" | Set-Content $Log
Remove-Item "$Out\$Mode-windows.stderr.txt"
Add-Content $Log "exit $code"
Get-Content $Log | Where-Object { $_ -match 'CHECK|^exit' -and $_ -notmatch 'protocol request|CHECK report|page-load' }
Remove-Item -Recurse -Force $S -ErrorAction SilentlyContinue
exit $code
