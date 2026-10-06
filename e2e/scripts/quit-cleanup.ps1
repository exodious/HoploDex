<#
.SYNOPSIS
Checks FR-035 / SC-010 on the real app on Windows: a decrypted copy of an
opened document must be gone once the app exits, however it is asked to quit.

.DESCRIPTION
The Windows counterpart of quit-cleanup.py (#27). The WebDriver E2E suite
doesn't check this: its harness kills the app (e2e/support/app.ts) without
letting it run its exit handler, so the wdio specs verify the crash and
relaunch half (the startup sweep) and this script the exit half.

It launches the app with a scratch sandbox (never the real directories),
plants a file where `open_document` writes its temporary copies, asks the
app to quit, and looks at what is left. Windows has no termination signals
for a windowed app; it asks in these ways instead:
  - window closed: WM_CLOSE to the app's window, as its close button does.
  - taskkill: `taskkill /PID` without /F, which sends WM_CLOSE to every
    top-level window of the process, the hidden system-messages one too.
  - log-off: WM_QUERYENDSESSION and then WM_ENDSESSION to every top-level
    window, as Windows sends them at a log-off or shutdown. Windows may end
    the process as soon as they are answered, so the script then ends it
    at once, and the copy must already be gone.
  - TerminateProcess (control): nothing runs, so the copy stays for the
    next launch's sweep.

Build the E2E binary first, then run it in a desktop session:

  npm run build
  cargo build --profile e2e --features custom-protocol,e2e --manifest-path src-tauri/Cargo.toml
  powershell -ExecutionPolicy Bypass -File e2e\scripts\quit-cleanup.ps1

.PARAMETER Binary
The E2E build to check. Defaults to HOPLODEX_BIN, or
src-tauri\target\e2e\hoplodex.exe.
#>
#Requires -Version 5.1
[CmdletBinding()]
param(
    [string]$Binary
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$Root = Resolve-Path (Join-Path $PSScriptRoot '..\..')
if (-not $Binary) {
    $Binary = if ($env:HOPLODEX_BIN) { $env:HOPLODEX_BIN } else { Join-Path $Root 'src-tauri\target\e2e\hoplodex.exe' }
}
$WindowTitle = 'HoploDex'
$Identifier = 'io.github.exodious.HoploDex'
# Clear of the E2E workers' ports (4445 up, e2e/support/app.ts).
$WebDriverPort = 4440

Add-Type -TypeDefinition @'
using System;
using System.Collections.Generic;
using System.Runtime.InteropServices;
using System.Text;

public static class QuitCleanupWindows {
    public const uint WM_CLOSE = 0x0010;
    const uint WM_QUERYENDSESSION = 0x0011;
    const uint WM_ENDSESSION = 0x0016;
    const uint ENDSESSION_LOGOFF = 0x80000000;
    const uint SMTO_ABORTIFHUNG = 0x0002;
    // src-tauri/src/platform/windows.rs
    const string SystemMessages = "HoploDexSystemMessages";

    delegate bool EnumWindowsProc(IntPtr window, IntPtr parameter);

    [DllImport("user32.dll")]
    static extern bool EnumWindows(EnumWindowsProc callback, IntPtr parameter);
    [DllImport("user32.dll")]
    static extern uint GetWindowThreadProcessId(IntPtr window, out uint processId);
    [DllImport("user32.dll")]
    static extern bool IsWindow(IntPtr window);
    [DllImport("user32.dll")]
    static extern bool IsWindowVisible(IntPtr window);
    [DllImport("user32.dll", CharSet = CharSet.Unicode)]
    static extern int GetWindowText(IntPtr window, StringBuilder text, int capacity);
    [DllImport("user32.dll", CharSet = CharSet.Unicode)]
    static extern int GetClassName(IntPtr window, StringBuilder name, int capacity);
    [DllImport("user32.dll")]
    static extern bool PostMessage(IntPtr window, uint message, UIntPtr wparam, IntPtr lparam);
    [DllImport("user32.dll")]
    static extern IntPtr SendMessageTimeout(IntPtr window, uint message, UIntPtr wparam,
        IntPtr lparam, uint flags, uint timeout, out UIntPtr result);

    /// The process's top-level windows, shown or not.
    public static List<IntPtr> TopLevel(int processId) {
        var found = new List<IntPtr>();
        EnumWindows((window, _) => {
            uint owner;
            GetWindowThreadProcessId(window, out owner);
            if (owner == processId) found.Add(window);
            return true;
        }, IntPtr.Zero);
        return found;
    }

    /// The process's shown top-level window with this title, or zero.
    public static IntPtr Shown(int processId, string title) {
        foreach (var window in TopLevel(processId)) {
            var text = new StringBuilder(256);
            GetWindowText(window, text, text.Capacity);
            if (IsWindowVisible(window) && text.ToString() == title) return window;
        }
        return IntPtr.Zero;
    }

    public static string ClassOf(IntPtr window) {
        var name = new StringBuilder(256);
        GetClassName(window, name, name.Capacity);
        return name.ToString();
    }

    public static void Post(IntPtr window, uint message) {
        PostMessage(window, message, UIntPtr.Zero, IntPtr.Zero);
    }

    /// What Windows sends at a log-off: WM_QUERYENDSESSION to every
    /// top-level window, then WM_ENDSESSION. Returns null once each has
    /// answered, or gone as the app exits, or else what went wrong. The
    /// hidden system-messages window, whose answer waits for the app's
    /// close, comes last, so the process can be ended as soon as it answers.
    public static string EndSession(int processId) {
        var windows = TopLevel(processId);
        windows.Sort((a, b) => (ClassOf(a) == SystemMessages).CompareTo(ClassOf(b) == SystemMessages));
        foreach (var window in windows) {
            var answer = Send(window, WM_QUERYENDSESSION, 0, ENDSESSION_LOGOFF, 5);
            if (answer == 0) return ClassOf(window) + " refused the log-off";
            if (answer == null && IsWindow(window)) return ClassOf(window) + " never answered the log-off";
        }
        foreach (var window in windows) {
            var answer = Send(window, WM_ENDSESSION, 1, ENDSESSION_LOGOFF, 10);
            if (answer == null && IsWindow(window)) return ClassOf(window) + " never answered the log-off";
        }
        return null;
    }

    /// Sends and waits, as Windows does at a session end, up to `seconds`.
    /// Returns what the window answered, or null if it didn't in time or
    /// is gone.
    static long? Send(IntPtr window, uint message, uint wparam, uint lparam, uint seconds) {
        UIntPtr result;
        var sent = SendMessageTimeout(window, message, new UIntPtr(wparam),
            new IntPtr((long)lparam), SMTO_ABORTIFHUNG, seconds * 1000, out result);
        if (sent == IntPtr.Zero) return null;
        return (long)result.ToUInt64();
    }
}
'@

function Invoke-Quit([string]$How) {
    $scratch = Join-Path ([IO.Path]::GetTempPath()) ("hoplodex-quit-" + [guid]::NewGuid().ToString('N'))
    $config, $cache, $documents, $webview = 'config', 'cache', 'documents', 'webview2' |
        ForEach-Object { New-Item -ItemType Directory -Path (Join-Path $scratch $_) | Select-Object -ExpandProperty FullName }
    # An E2E build takes its directories from these, never from the OS
    # (src-tauri/src/app_dirs.rs), and won't start without them.
    $env:HOPLODEX_E2E_CONFIG_HOME = $config
    $env:HOPLODEX_E2E_CACHE_HOME = $cache
    $env:HOPLODEX_E2E_DOCUMENTS = $documents
    $env:WEBVIEW2_USER_DATA_FOLDER = $webview
    $env:TAURI_WEBDRIVER_PORT = "$WebDriverPort"
    $planted = Join-Path $cache "$Identifier\opened-documents\7\receipt.pdf"

    $app = Start-Process -FilePath $Binary -PassThru
    try {
        $deadline = (Get-Date).AddSeconds(30)
        while ((Get-Date) -lt $deadline -and [QuitCleanupWindows]::Shown($app.Id, $WindowTitle) -eq [IntPtr]::Zero) {
            Start-Sleep -Milliseconds 500
        }
        $window = [QuitCleanupWindows]::Shown($app.Id, $WindowTitle)
        if ($window -eq [IntPtr]::Zero) { return $false, "the app's window never appeared" }
        Start-Sleep -Seconds 3  # let startup (and its sweep of old copies) finish first

        New-Item -ItemType Directory -Path (Split-Path $planted) | Out-Null
        Set-Content -Path $planted -Value '%PDF-1.4 decrypted copy' -NoNewline

        switch ($How) {
            'close' { [QuitCleanupWindows]::Post($window, [QuitCleanupWindows]::WM_CLOSE) }
            'taskkill' {
                & taskkill.exe /PID $app.Id | Out-Null
                if ($LASTEXITCODE -ne 0) { return $false, "taskkill failed with exit code $LASTEXITCODE" }
            }
            'logoff' {
                $problem = [QuitCleanupWindows]::EndSession($app.Id)
                if ($problem) { return $false, $problem }
                # Every window has answered, so Windows may end the process now.
                Stop-Process -Id $app.Id -Force -ErrorAction SilentlyContinue
            }
            'kill' { Stop-Process -Id $app.Id -Force }
        }
        if (-not $app.WaitForExit(20000)) { return $false, "the app didn't exit within 20s" }
        Start-Sleep -Milliseconds 500
        $left = Test-Path $planted
        if ($How -eq 'kill') {
            # A killed process can't clean up; the next launch's sweep does.
            return $left, $(if ($left) { 'left behind for the next launch to sweep' } else { 'was somehow cleaned up' })
        }
        return (-not $left), $(if ($left) { 'still on disk after exit' } else { 'removed on exit' })
    } finally {
        if (-not $app.HasExited) { Stop-Process -Id $app.Id -Force -ErrorAction SilentlyContinue }
        # The webview's processes can hold its folder a moment longer.
        for ($attempt = 0; $attempt -lt 10 -and (Test-Path $scratch); $attempt++) {
            Start-Sleep -Milliseconds 500
            Remove-Item -Recurse -Force $scratch -ErrorAction SilentlyContinue
        }
    }
}

if (-not (Test-Path $Binary)) {
    Write-Error "build the E2E binary first: $Binary not found"
    exit 2
}
$failures = 0
foreach ($case in @(
        @('close', 'window closed'),
        @('taskkill', 'taskkill without /F'),
        @('logoff', 'log-off (WM_QUERYENDSESSION, WM_ENDSESSION)'),
        @('kill', 'TerminateProcess (control)'))) {
    $ok, $detail = Invoke-Quit $case[0]
    Write-Host ("{0}  {1}: {2}" -f $(if ($ok) { 'PASS' } else { 'FAIL' }), $case[1], $detail)
    if (-not $ok) { $failures++ }
}
exit $(if ($failures) { 1 } else { 0 })
