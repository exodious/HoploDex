# For scripts/windows/pdf-surface-check.ps1: real mouse input, screenshots
# and window bounds on Windows, as pdf_surface_input.swift gives the macOS
# runs and pdf_surface_input.py the Linux ones. Run it in the signed-in desktop
# session (an SSH session has no desktop to post input to).
#
#   pdf_surface_input.ps1 bounds PID       the PID's largest visible window:
#                                        x y width height (physical pixels)
#   pdf_surface_input.ps1 move X Y         move the pointer
#   pdf_surface_input.ps1 click X Y        left click
#   pdf_surface_input.ps1 rclick X Y       right click
#   pdf_surface_input.ps1 scroll X Y N     wheel N notches at X Y (negative: down)
#   pdf_surface_input.ps1 key KEYS         send keys (SendKeys syntax, e.g. ^s)
#   pdf_surface_input.ps1 shot FILE        screenshot of the whole screen (PNG)
#   pdf_surface_input.ps1 text PID         every name and value the PID's UI
#                                        Automation tree exposes (what a screen
#                                        reader could read), with each
#                                        element's control type and position
#   pdf_surface_input.ps1 press PID NAME   invoke the PID's element named NAME
#                                        (a button, a menu item)
param(
    [Parameter(Mandatory)][string]$Command,
    [Parameter(ValueFromRemainingArguments)][string[]]$Rest = @()
)
$ErrorActionPreference = 'Stop'

Add-Type -ReferencedAssemblies System.Drawing -TypeDefinition @'
using System;
using System.Collections.Generic;
using System.Runtime.InteropServices;
using System.Threading;

public static class SurfaceInput {
    [DllImport("user32.dll")] static extern bool SetProcessDpiAwarenessContext(IntPtr v);
    [DllImport("user32.dll")] static extern bool SetCursorPos(int x, int y);
    [DllImport("user32.dll")] static extern void mouse_event(uint f, int x, int y, uint d, UIntPtr e);
    [DllImport("user32.dll")] static extern int GetSystemMetrics(int i);
    delegate bool EnumProc(IntPtr h, IntPtr l);
    [DllImport("user32.dll")] static extern bool EnumWindows(EnumProc p, IntPtr l);
    [DllImport("user32.dll")] static extern uint GetWindowThreadProcessId(IntPtr h, out uint pid);
    [DllImport("user32.dll")] static extern bool IsWindowVisible(IntPtr h);
    [DllImport("user32.dll")] static extern bool GetWindowRect(IntPtr h, out RECT r);
    [DllImport("user32.dll")] static extern bool SetForegroundWindow(IntPtr h);
    public struct RECT { public int L, T, R, B; }

    public static void DpiAware() { SetProcessDpiAwarenessContext(new IntPtr(-4)); }

    public static int[] Bounds(uint pid) {
        int[] best = null; long area = 0; IntPtr found = IntPtr.Zero;
        EnumWindows((h, l) => {
            uint p; GetWindowThreadProcessId(h, out p);
            RECT r;
            if (p == pid && IsWindowVisible(h) && GetWindowRect(h, out r)) {
                long a = (long)(r.R - r.L) * (r.B - r.T);
                if (a > area) { area = a; best = new[] { r.L, r.T, r.R - r.L, r.B - r.T }; found = h; }
            }
            return true;
        }, IntPtr.Zero);
        if (found != IntPtr.Zero) SetForegroundWindow(found);
        return best;
    }

    public static void Move(int x, int y) { SetCursorPos(x, y); Thread.Sleep(80); }
    public static void Click(int x, int y, bool right) {
        Move(x, y);
        mouse_event(right ? 0x0008u : 0x0002u, 0, 0, 0, UIntPtr.Zero); Thread.Sleep(60);
        mouse_event(right ? 0x0010u : 0x0004u, 0, 0, 0, UIntPtr.Zero); Thread.Sleep(80);
    }

    public static void Scroll(int x, int y, int notches) {
        Move(x, y);
        for (int i = 0; i < Math.Abs(notches); i++) {
            mouse_event(0x0800u, 0, 0, unchecked((uint)(notches > 0 ? 120 : -120)), UIntPtr.Zero);
            Thread.Sleep(40);
        }
    }

    public static void Shot(string file) {
        int w = GetSystemMetrics(0), h = GetSystemMetrics(1);
        using (var bmp = new System.Drawing.Bitmap(w, h))
        using (var g = System.Drawing.Graphics.FromImage(bmp)) {
            g.CopyFromScreen(0, 0, 0, 0, bmp.Size);
            bmp.Save(file, System.Drawing.Imaging.ImageFormat.Png);
        }
    }
}
'@
[SurfaceInput]::DpiAware()

function Walk-Tree([uint32]$ProcessId, [scriptblock]$Visit) {
    Add-Type -AssemblyName UIAutomationClient, UIAutomationTypes
    $A = [System.Windows.Automation.AutomationElement]
    $cond = New-Object System.Windows.Automation.PropertyCondition($A::ProcessIdProperty, [int]$ProcessId)
    $walker = [System.Windows.Automation.TreeWalker]::RawViewWalker
    $stack = New-Object System.Collections.Stack
    foreach ($top in $A::RootElement.FindAll([System.Windows.Automation.TreeScope]::Children, $cond)) { $stack.Push(@($top, 0)) }
    while ($stack.Count -gt 0) {
        $e, $depth = $stack.Pop()
        try { $info = $e.Current } catch { continue }
        $value = ''
        try { $value = $e.GetCurrentPropertyValue($A::ValueProperty::Value) } catch {}
        & $Visit $e $info $value
        if ($depth -ge 80) { continue }
        $kids = @()
        $c = $walker.GetFirstChild($e)
        while ($c) { $kids += $c; $c = $walker.GetNextSibling($c) }
        [array]::Reverse($kids)
        foreach ($k in $kids) { $stack.Push(@($k, ($depth + 1))) }
    }
}

switch ($Command) {
    'bounds' {
        $b = [SurfaceInput]::Bounds([uint32]$Rest[0])
        if (-not $b) { exit 1 }
        "$($b[0]) $($b[1]) $($b[2]) $($b[3])"
    }
    'move' { [SurfaceInput]::Move([int]$Rest[0], [int]$Rest[1]) }
    'click' { [SurfaceInput]::Click([int]$Rest[0], [int]$Rest[1], $false) }
    'rclick' { [SurfaceInput]::Click([int]$Rest[0], [int]$Rest[1], $true) }
    'key' {
        Add-Type -AssemblyName System.Windows.Forms
        [System.Windows.Forms.SendKeys]::SendWait($Rest[0])
    }
    'scroll' { [SurfaceInput]::Scroll([int]$Rest[0], [int]$Rest[1], [int]$Rest[2]) }
    'shot' { [SurfaceInput]::Shot($Rest[0]) }
    'text' {
        Walk-Tree $Rest[0] {
            param($e, $info, $value)
            $strings = @($info.Name, $value, $info.HelpText) | Where-Object { $_ } | ForEach-Object { $_.Substring(0, [Math]::Min(100, $_.Length)) }
            $type = $info.ControlType.ProgrammaticName -replace '^ControlType\.', ''
            if ($strings -or $type -eq 'Button') {
                $r = $info.BoundingRectangle
                $at = if ($r.IsEmpty) { '' } else { " @$([int]$r.X),$([int]$r.Y)" }
                "$type$at [$($strings -join ' | ')]"
            }
        }
    }
    'press' {
        $script:pressed = $false
        Walk-Tree $Rest[0] {
            param($e, $info, $value)
            if (-not $script:pressed -and $info.Name -eq $Rest[1]) {
                foreach ($p in $e.GetSupportedPatterns()) {
                    if ($p.ProgrammaticName -eq 'InvokePatternIdentifiers.Pattern') {
                        $e.GetCurrentPattern($p).Invoke(); $script:pressed = $true; break
                    }
                }
            }
        }
        if ($script:pressed) { 'pressed' } else { 'not found' }
    }
    default {
        [Console]::Error.WriteLine('usage: bounds PID | move X Y | click X Y | rclick X Y | scroll X Y N | key KEYS | shot FILE | text PID | press PID NAME')
        exit 2
    }
}
