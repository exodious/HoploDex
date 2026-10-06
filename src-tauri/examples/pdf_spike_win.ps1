# Runs examples/pdf_spike.rs on Windows with real input, as pdf_spike.sh
# does on Linux and pdf_spike_mac.sh on macOS: takes screenshots, clicks
# Edge's PDF toolbar (its settings menu, Save, which saves through the OS's
# Save As dialog to the run's own folder, and Print), presses Ctrl+S,
# right-clicks for the context menu, clicks the PDF's link and reads the
# window's UI Automation
# tree. Then it lists every file written during the run under the user's
# AppData (the temp dir and WebView2's user data folder are in it) and any
# that holds the PDF's marker. Results:
# specs/007-document-preview/spike-webview-pdf.md.
#
#   powershell -ExecutionPolicy Bypass -File src-tauri\examples\pdf_spike_win.ps1 NAME [VAR=1 ...]
#
# Run it in the signed-in desktop session (the auto-logon session of
# DEVELOPMENT.md's Windows setup; an SSH session has no desktop), at 100%
# scaling: the clicks are fixed offsets from the window's corner. WebView2's
# user data folder is a fresh one in the run's temp folder
# (WEBVIEW2_USER_DATA_FOLDER), never the app's. SPIKE_SCAN_HOME=1 also
# searches the whole user profile (a test account's; the default is AppData).
#
# NAME labels the outputs in e2e\screenshots-out\pdf-spike\; the VARs are the
# example's SPIKE_* settings.
param([string]$Name = 'default', [Parameter(ValueFromRemainingArguments)][string[]]$Vars = @())
$ErrorActionPreference = 'Stop'
$Root = (Resolve-Path "$PSScriptRoot\..\..").Path
$Out = "$Root\e2e\screenshots-out\pdf-spike"
New-Item -ItemType Directory -Force $Out | Out-Null
$Input_ = "$PSScriptRoot\pdf_spike_input.ps1"
function send { & $Input_ @args }
function shot($label) { send shot "$Out\$Name-$label.png" }
function note($text) { Add-Content -Path $Log -Value "SPIKE $text" }

Push-Location "$Root\src-tauri"
cmd /c "cargo build --example pdf_spike 2>&1" | Select-Object -Last 1
Pop-Location

# The long form: SendKeys reads the ~ of a short (8.3) path as Enter.
$S = (New-Item -ItemType Directory -Force "$((Get-Item $env:TEMP).FullName)\spike-$([guid]::NewGuid().ToString('N').Substring(0, 6))").FullName
New-Item -ItemType Directory -Force "$S\wv", "$S\dl" | Out-Null
$Log = "$Out\$Name.log"
Set-Content -Path $Log -Value $null
$Stamp = Get-Date
Start-Sleep 1

$env:WEBVIEW2_USER_DATA_FOLDER = "$S\wv"
$env:SPIKE_DOWNLOAD_DIR = "$S\dl"
$env:SPIKE_EXIT_SECONDS = '75'
foreach ($v in $Vars) {
    $k, $val = $v -split '=', 2
    Set-Item "env:$k" $val
}
# Stderr goes to a file of its own (Start-Process won't append to the log),
# added to the log, after the steps' notes, once the spike has quit.
$P = Start-Process -FilePath "$Root\src-tauri\target\debug\examples\pdf_spike.exe" -PassThru `
    -RedirectStandardError "$Out\$Name.stderr.txt" -RedirectStandardOutput "$Out\$Name.stdout.txt"
Start-Sleep 12
# Points from the window's top left (with its invisible resize border): the
# window is 1000x800 and the viewer's layout is fixed, so these hold wherever
# Windows puts it.
$X, $Y = (send bounds $P.Id) -split ' ' | Select-Object -First 2 | ForEach-Object { [int]$_ }
function click($dx, $dy) { send click ($X + $dx) ($Y + $dy) }
function rclick($dx, $dy) { send rclick ($X + $dx) ($Y + $dy) }
click 700 600 # focus, on the page
shot 1
send text $P.Id | Out-Null # the first query turns on the web content's tree
Start-Sleep 2

click 986 50; note 'clicked the toolbar settings'
Start-Sleep 1.5
shot 2-settings
send key '{ESC}'; Start-Sleep 0.5

click 905 50; note 'clicked the toolbar Save'
Start-Sleep 2.5
shot 3-save
# Into the run's own folder, if the Save As dialog is up.
send key "$S\dl\chosen.pdf"; Start-Sleep 0.5; send key '{ENTER}'
Start-Sleep 3

rclick 400 450; Start-Sleep 1.5
shot 4-menu
send key '{ESC}'; Start-Sleep 0.5

click 869 50; note 'clicked the toolbar Print'
Start-Sleep 4
shot 5-print
send key '{ESC}'; Start-Sleep 1.5

click 700 600; send key '^s'; note 'pressed Ctrl+S'
Start-Sleep 2.5
shot 6-ctrl-s
send key '{ESC}'; Start-Sleep 1

click 318 321; note 'clicked the link'
Start-Sleep 3
shot 7-link
send text $P.Id | ForEach-Object { "SPIKE UIA $_" } | Add-Content -Path $Log
$P.WaitForExit()
note 'process ended'
Get-Content "$Out\$Name.stderr.txt" | Add-Content -Path $Log
Remove-Item "$Out\$Name.stderr.txt", "$Out\$Name.stdout.txt"

# Every file written during the run, and those holding the marker (as ASCII
# or UTF-16), leaving out the build, the outputs and the Save the run made.
$Scan = if ($env:SPIKE_SCAN_HOME -eq '1') { @($env:USERPROFILE) } else { @($env:LOCALAPPDATA, $env:APPDATA) }
$Skip = @("$Root\src-tauri\target", "$Root\node_modules", $Out, "$S\dl") | ForEach-Object { $_.ToLowerInvariant() }
$Written = $Scan | ForEach-Object { Get-ChildItem -LiteralPath $_ -Recurse -File -Force -ErrorAction SilentlyContinue } |
    Where-Object { $_.LastWriteTime -gt $Stamp } |
    Where-Object { $f = $_.FullName.ToLowerInvariant(); -not ($Skip | Where-Object { $f.StartsWith($_) }) }
$Ascii = [Text.Encoding]::GetEncoding(28591)
$Marker = 'HDSPIKEMARKER'
$Wide = $Ascii.GetString([Text.Encoding]::Unicode.GetBytes($Marker))
$Holding = $Written | Where-Object { $_.Length -lt 512MB } | Where-Object {
    try {
        $stream = [IO.File]::Open($_.FullName, 'Open', 'Read', 'ReadWrite, Delete')
        try {
            $bytes = New-Object byte[] $stream.Length
            [void]$stream.Read($bytes, 0, $bytes.Length)
        } finally { $stream.Dispose() }
        $text = $Ascii.GetString($bytes)
        $text.Contains($Marker) -or $text.Contains($Wide)
    } catch { $false }
}
$Disk = @('== files written during the run ==') + ($Written | ForEach-Object FullName) +
    @('== of those, files holding the marker ==') + ($Holding | ForEach-Object FullName) +
    @('== the Save the run made ==') + (Get-ChildItem "$S\dl" | ForEach-Object { "$($_.FullName) $($_.Length)" })
$Disk | Set-Content -Path "$Out\$Name.disk.txt"
Get-Content $Log | Where-Object { $_ -match 'SPIKE' -and $_ -notmatch 'protocol request|SPIKE UIA' }
$Disk | Select-Object -Skip ([array]::IndexOf($Disk, '== of those, files holding the marker =='))
