# The window controls check on Windows (specs/008-hardening-batch/research.md
# §13, §14, §18, 008 quickstart.md): builds src-tauri\examples\window_controls_check.rs
# with the E2E profile (a development build installs none of the filters,
# 008 FR-014) and runs it with real input. It opens the main window with the app's
# own `window_controls::apply` (WebView2's context menu filter, browser
# accelerator keys and swipe navigation off) and a test page, right-clicks a
# blank area, a row, a button, an image, a text field and a selection and logs
# every item each menu offered, presses every 008 FR-012 key (the Browser Back,
# Forward and Refresh keys by virtual key code) and the mouse's back and
# forward buttons, and fails if a menu offered an item outside 008 FR-011's list or
# appeared where none belongs, or the page reloaded or navigated.
#
#   powershell -ExecutionPolicy Bypass -File scripts\windows\window-controls-check.ps1
#
# Run it in the signed-in desktop session (the auto-logon session of
# DEVELOPMENT.md's Windows setup; an SSH session has no desktop to post input
# to or take a screenshot of), at 100% scaling. The app's folders come from the
# E2E variables (an `e2e` build) and are a fresh scratch folder's, and the main
# web view's WebView2 user data folder is its own under it. The log and the
# screenshot go to e2e\screenshots-out\window-controls\. Exits with the
# example's code (0 passed, 1 no window, 3 usage, 4 a check failed).
#
# Run on the Windows VM for 008 T085.
param([Parameter(ValueFromRemainingArguments)][string[]]$Rest = @())
$ErrorActionPreference = 'Stop'
$Root = (Resolve-Path "$PSScriptRoot\..\..").Path
$Out = "$Root\e2e\screenshots-out\window-controls"
New-Item -ItemType Directory -Force $Out | Out-Null

Push-Location "$Root\src-tauri"
cmd /c "cargo build --profile e2e --example window_controls_check --features e2e 2>&1"
if ($LASTEXITCODE -ne 0) { exit 1 }
Pop-Location

# The long form of the temp path: SendKeys reads the ~ of a short (8.3) path
# as Enter.
$S = (New-Item -ItemType Directory -Force "$((Get-Item $env:TEMP).FullName)\window-controls-$([guid]::NewGuid().ToString('N').Substring(0, 6))").FullName
New-Item -ItemType Directory -Force "$S\config", "$S\cache", "$S\documents" | Out-Null
$Log = "$Out\window-controls-windows.log"
$env:HOPLODEX_E2E_CONFIG_HOME = "$S\config"
$env:HOPLODEX_E2E_CACHE_HOME = "$S\cache"
$env:HOPLODEX_E2E_DOCUMENTS = "$S\documents"

$input_ = "$Root\src-tauri\examples\real_input.ps1"
$flags = @('--input', 'powershell', '--input', '-NoProfile', '--input', '-ExecutionPolicy',
    '--input', 'Bypass', '--input', '-File', '--input', $input_)
$exe = "$Root\src-tauri\target\e2e\examples\window_controls_check.exe"
$p = Start-Process -FilePath $exe -PassThru -NoNewWindow -RedirectStandardError "$Out\window-controls-windows.stderr.txt" `
    -ArgumentList (@('--scratch', "$S\scratch", '--out', $Out) + $flags + $Rest)
# Windows PowerShell 5.1 reads a null ExitCode unless the process's handle was
# taken while it ran.
$null = $p.Handle
$p.WaitForExit()
$code = $p.ExitCode
Get-Content "$Out\window-controls-windows.stderr.txt" | Set-Content $Log
Remove-Item "$Out\window-controls-windows.stderr.txt"
Add-Content $Log "exit $code"
Get-Content $Log | Where-Object { $_ -match 'CHECK|^exit' }
Remove-Item -Recurse -Force $S -ErrorAction SilentlyContinue
exit $code
