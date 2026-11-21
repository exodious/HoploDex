# For scripts/windows/drop-check.ps1's app drop check (examples/
# app_drop_check.rs): shows a file in File Explorer, where a user drags it
# from (#207). Explorer's data object isn't the one drag_source.ps1 makes:
# among other things it answers S_FALSE, not an error, when asked for a
# format it doesn't have.
#
#   explorer_source.ps1 FILE [X Y WIDTH HEIGHT]
#
# Opens an Explorer window on FILE's folder with FILE selected, at X Y (screen
# pixels, default 0 300) and WIDTH by HEIGHT (default 560 by 420), above other
# windows, and prints the screen point to drag FILE from: "X Y". It then waits
# for its standard input to close, and closes that window. Run it in the
# signed-in desktop session.
param(
    [Parameter(Mandatory)][string]$File,
    [int]$X = 0, [int]$Y = 300, [int]$Width = 560, [int]$Height = 420
)
$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName UIAutomationClient, UIAutomationTypes
Add-Type @'
using System; using System.Runtime.InteropServices;
public static class ExplorerSource {
    [DllImport("user32.dll")] public static extern bool SetProcessDpiAwarenessContext(IntPtr v);
    [DllImport("user32.dll")] public static extern bool SetWindowPos(IntPtr h, IntPtr after, int x, int y, int w, int hh, uint f);
    [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr h);
}
'@
[ExplorerSource]::SetProcessDpiAwarenessContext([IntPtr]::new(-4)) | Out-Null
$A = [System.Windows.Automation.AutomationElement]
$Scope = [System.Windows.Automation.TreeScope]
$name = Split-Path -Leaf $File
$folder = Split-Path -Leaf (Split-Path -Parent $File)

# Explorer's windows on the folder, which are titled with its name.
function Get-FolderWindows {
    $cond = New-Object System.Windows.Automation.PropertyCondition($A::ClassNameProperty, 'CabinetWClass')
    @($A::RootElement.FindAll($Scope::Children, $cond) | Where-Object { $_.Current.Name -like "$folder*" })
}
function Close-FolderWindows {
    foreach ($w in Get-FolderWindows) {
        try { $w.GetCurrentPattern([System.Windows.Automation.WindowPattern]::Pattern).Close() } catch {}
    }
}

# One window on the folder: the one opened here.
Close-FolderWindows
Start-Process explorer.exe -ArgumentList "/select,`"$File`""
$deadline = (Get-Date).AddSeconds(30)
$item = $null
while (-not $item -and (Get-Date) -lt $deadline) {
    Start-Sleep -Milliseconds 500
    $window = Get-FolderWindows | Select-Object -First 1
    if (-not $window) { continue }
    $hwnd = [IntPtr]$window.Current.NativeWindowHandle
    # HWND_TOPMOST, SWP_SHOWWINDOW
    [ExplorerSource]::SetWindowPos($hwnd, [IntPtr]::new(-1), $X, $Y, $Width, $Height, 0x40) | Out-Null
    [ExplorerSource]::SetForegroundWindow($hwnd) | Out-Null
    Start-Sleep -Milliseconds 800
    # Explorer hides known extensions unless told otherwise.
    foreach ($n in @($name, [IO.Path]::GetFileNameWithoutExtension($name))) {
        $cond = New-Object System.Windows.Automation.AndCondition(
            (New-Object System.Windows.Automation.PropertyCondition($A::NameProperty, $n)),
            (New-Object System.Windows.Automation.PropertyCondition($A::ControlTypeProperty, [System.Windows.Automation.ControlType]::ListItem)))
        if (-not $item) { $item = $window.FindFirst($Scope::Descendants, $cond) }
    }
}
if (-not $item) {
    [Console]::Error.WriteLine("explorer_source: Explorer showed no $name")
    Close-FolderWindows
    exit 1
}
# Its icon, near the item's left edge, in a view with or without details.
$r = $item.Current.BoundingRectangle
[Console]::Out.WriteLine("$([int]($r.X + [Math]::Min(40, $r.Width / 2))) $([int]($r.Y + $r.Height / 2))")
[Console]::Out.Flush()
$null = [Console]::In.ReadToEnd()
Close-FolderWindows
