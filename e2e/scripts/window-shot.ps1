<#
.SYNOPSIS
Saves the E2E app's window, as the user sees it, on Windows (#27).

.DESCRIPTION
The Windows counterpart of the X display's `import -window root` that
e2e/specs/us13-document-preview.e2e.ts and e2e/support/screenshots.ts use on
Linux. A WebDriver screenshot holds only the main web view, so it leaves out
007's PDF surface, a child web view over the viewer's page area; this copies
what is on the screen, which has it. It takes the window's client area (what
the main web view fills), so a point in the page, as getBoundingClientRect()
gives it, is the same point in the picture, whatever the window's frame and
position.

Run it in the signed-in desktop session, at 100% scaling, with the app's window
on top and not covered. Exits 1 when the app has no window.

.PARAMETER Out
The file to write.

.PARAMETER Format
`png` (the default) or `ppm`: a binary P6, 8 bits a channel, which the specs
read themselves.
#>
#Requires -Version 5.1
[CmdletBinding()]
param(
    [Parameter(Mandatory)][string]$Out,
    [ValidateSet('png', 'ppm')][string]$Format = 'png'
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

Add-Type -AssemblyName System.Drawing
Add-Type -ReferencedAssemblies System.Drawing -TypeDefinition @'
using System;
using System.Drawing;
using System.Drawing.Imaging;
using System.IO;
using System.Runtime.InteropServices;

public static class WindowShot {
    [StructLayout(LayoutKind.Sequential)] struct RECT { public int Left, Top, Right, Bottom; }
    [StructLayout(LayoutKind.Sequential)] struct POINT { public int X, Y; }
    [DllImport("user32.dll")] static extern bool SetProcessDPIAware();
    [DllImport("user32.dll")] static extern bool GetClientRect(IntPtr hwnd, out RECT rect);
    [DllImport("user32.dll")] static extern bool ClientToScreen(IntPtr hwnd, ref POINT point);
    [DllImport("user32.dll")] static extern bool SetForegroundWindow(IntPtr hwnd);

    public static void Save(IntPtr hwnd, string file, bool ppm) {
        SetProcessDPIAware();
        SetForegroundWindow(hwnd);
        RECT client;
        if (!GetClientRect(hwnd, out client)) throw new Exception("GetClientRect failed");
        POINT origin = new POINT();
        if (!ClientToScreen(hwnd, ref origin)) throw new Exception("ClientToScreen failed");
        int w = client.Right - client.Left, h = client.Bottom - client.Top;
        using (Bitmap bmp = new Bitmap(w, h, PixelFormat.Format32bppArgb))
        {
            using (Graphics g = Graphics.FromImage(bmp)) g.CopyFromScreen(origin.X, origin.Y, 0, 0, bmp.Size);
            if (!ppm) { bmp.Save(file, ImageFormat.Png); return; }
            BitmapData data = bmp.LockBits(new Rectangle(0, 0, w, h), ImageLockMode.ReadOnly, PixelFormat.Format32bppArgb);
            try {
                byte[] bgra = new byte[data.Stride * h];
                Marshal.Copy(data.Scan0, bgra, 0, bgra.Length);
                byte[] rgb = new byte[w * h * 3];
                for (int y = 0; y < h; y++) {
                    for (int x = 0; x < w; x++) {
                        int s = y * data.Stride + x * 4, d = (y * w + x) * 3;
                        rgb[d] = bgra[s + 2]; rgb[d + 1] = bgra[s + 1]; rgb[d + 2] = bgra[s];
                    }
                }
                using (FileStream f = File.Create(file)) {
                    byte[] head = System.Text.Encoding.ASCII.GetBytes("P6\n" + w + " " + h + "\n255\n");
                    f.Write(head, 0, head.Length);
                    f.Write(rgb, 0, rgb.Length);
                }
            } finally { bmp.UnlockBits(data); }
        }
    }
}
'@

$window = Get-Process -Name hoplodex -ErrorAction SilentlyContinue |
    Where-Object { $_.MainWindowHandle -ne [IntPtr]::Zero } | Select-Object -First 1
if (-not $window) {
    [Console]::Error.WriteLine('window-shot: the app has no window')
    exit 1
}
[WindowShot]::Save($window.MainWindowHandle, $Out, $Format -eq 'ppm')
