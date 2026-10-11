<#
.SYNOPSIS
Saves the E2E app's window, as the user sees it, on Windows (#27, #88).

.DESCRIPTION
The Windows counterpart of the X display's `import -window root` that
e2e/specs/us13-document-preview.e2e.ts and e2e/support/screenshots.ts use on
Linux. A WebDriver screenshot holds only the main web view, so it leaves out
007's PDF surface, a child web view over the viewer's page area; this asks the
window to paint itself (PrintWindow with PW_RENDERFULLCONTENT, which includes
the child web views), so it has the surface. It takes the window's client
area (what the main web view fills), so a point in the page, as
getBoundingClientRect() gives it, is the same point in the picture, whatever
the window's frame and position.

It captures the window of one process, the E2E worker's own app (-ProcessId),
and not the screen, so the parallel workers' other windows, on top or beside
it, don't matter and nothing is brought to the front. Run it in the signed-in
desktop session, at 100% scaling. The window must not be minimized. Exits 1
when the process has no window.

.PARAMETER ProcessId
The app's process id (`appPid()` in e2e/support/app.ts).

.PARAMETER Out
The file to write.

.PARAMETER Format
`png` (the default) or `ppm`: a binary P6, 8 bits a channel, which the specs
read themselves.
#>
#Requires -Version 5.1
[CmdletBinding()]
param(
    [Parameter(Mandatory)][int]$ProcessId,
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
    [DllImport("user32.dll")] static extern bool SetProcessDPIAware();
    [DllImport("user32.dll")] static extern bool GetClientRect(IntPtr hwnd, out RECT rect);
    [DllImport("user32.dll")] static extern bool PrintWindow(IntPtr hwnd, IntPtr hdc, uint flags);
    const uint PW_CLIENTONLY = 1, PW_RENDERFULLCONTENT = 2;

    public static void Save(IntPtr hwnd, string file, bool ppm) {
        SetProcessDPIAware();
        RECT client;
        if (!GetClientRect(hwnd, out client)) throw new Exception("GetClientRect failed");
        int w = client.Right - client.Left, h = client.Bottom - client.Top;
        using (Bitmap bmp = new Bitmap(w, h, PixelFormat.Format32bppArgb))
        {
            using (Graphics g = Graphics.FromImage(bmp))
            {
                IntPtr hdc = g.GetHdc();
                bool ok;
                try { ok = PrintWindow(hwnd, hdc, PW_CLIENTONLY | PW_RENDERFULLCONTENT); }
                finally { g.ReleaseHdc(hdc); }
                if (!ok) throw new Exception("PrintWindow failed");
            }
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

# MainWindowHandle is zero until the process has shown a window, and is cached.
$handle = [IntPtr]::Zero
for ($try = 0; $try -lt 50 -and $handle -eq [IntPtr]::Zero; $try++) {
    $process = Get-Process -Id $ProcessId -ErrorAction SilentlyContinue
    if (-not $process) { break }
    $process.Refresh()
    $handle = $process.MainWindowHandle
    if ($handle -eq [IntPtr]::Zero) { Start-Sleep -Milliseconds 100 }
}
if ($handle -eq [IntPtr]::Zero) {
    [Console]::Error.WriteLine("window-shot: process $ProcessId has no window")
    exit 1
}
[WindowShot]::Save($handle, $Out, $Format -eq 'ppm')
