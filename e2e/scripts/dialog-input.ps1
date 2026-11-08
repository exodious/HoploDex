# Keys for the native passphrase dialog of one app process, on Windows
# (e2e/support/dialogInput.ts). Another app can take the foreground (another
# worker's, with HOPLODEX_E2E_WORKERS above 1), so keys sent to the foreground
# window (SendKeys) can reach it. These are posted to the dialog itself, as
# macOS's real_input.swift posts to one process (#122): to the control that has
# the focus on the dialog's thread, where its message loop's IsDialogMessageW
# handles Tab, Enter and Escape as it does for a key pressed on the keyboard.
#
#   dialog-input.ps1 -ProcessId PID -Text "some words"   type the text
#   dialog-input.ps1 -ProcessId PID -Key Return          Return, Escape, Tab, space or BackSpace
#   dialog-input.ps1 -ProcessId PID -Shot FILE.png       the dialog's window, as it paints itself
#                                                        (PrintWindow), whatever is over it
param(
    [Parameter(Mandatory)][int]$ProcessId,
    [string]$Text,
    [string]$Key,
    [string]$Shot
)
$ErrorActionPreference = 'Stop'

Add-Type -ReferencedAssemblies System.Drawing -TypeDefinition @'
using System;
using System.Runtime.InteropServices;
using System.Text;
using System.Threading;

public static class DialogInput {
    delegate bool EnumProc(IntPtr h, IntPtr l);
    [DllImport("user32.dll")] static extern bool EnumWindows(EnumProc p, IntPtr l);
    [DllImport("user32.dll")] static extern uint GetWindowThreadProcessId(IntPtr h, out uint pid);
    [DllImport("user32.dll", CharSet = CharSet.Unicode)] static extern int GetClassNameW(IntPtr h, StringBuilder s, int n);
    [DllImport("user32.dll")] static extern bool IsWindowVisible(IntPtr h);
    [DllImport("user32.dll")] static extern bool PostMessageW(IntPtr h, uint m, IntPtr w, IntPtr l);
    [DllImport("user32.dll")] static extern bool GetGUIThreadInfo(uint thread, ref GUITHREADINFO info);
    [DllImport("user32.dll", CharSet = CharSet.Unicode)] static extern IntPtr GetPropW(IntPtr h, string name);
    [StructLayout(LayoutKind.Sequential)] public struct RECT { public int L, T, R, B; }
    [StructLayout(LayoutKind.Sequential)] public struct GUITHREADINFO {
        public int cbSize, flags;
        public IntPtr active, focus, capture, menuOwner, moveSize, caret;
        public RECT caretRect;
    }
    const uint WM_KEYDOWN = 0x0100, WM_KEYUP = 0x0101, WM_CHAR = 0x0102;

    [DllImport("user32.dll")] static extern bool GetWindowRect(IntPtr h, out RECT r);
    [DllImport("user32.dll")] static extern bool PrintWindow(IntPtr h, IntPtr hdc, uint flags);
    [DllImport("user32.dll")] static extern bool SetProcessDPIAware();

    /// Saves the dialog's window as a PNG, painted by the window itself.
    public static void Shot(uint pid, string file) {
        SetProcessDPIAware();
        IntPtr dialog = Dialog(pid);
        RECT r; GetWindowRect(dialog, out r);
        using (var bitmap = new System.Drawing.Bitmap(r.R - r.L, r.B - r.T)) {
            using (var g = System.Drawing.Graphics.FromImage(bitmap)) {
                IntPtr hdc = g.GetHdc();
                PrintWindow(dialog, hdc, 2);
                g.ReleaseHdc(hdc);
            }
            bitmap.Save(file, System.Drawing.Imaging.ImageFormat.Png);
        }
    }

    /// The process's passphrase dialog.
    static IntPtr Dialog(uint pid) {
        IntPtr dialog = IntPtr.Zero;
        EnumWindows((h, l) => {
            uint p; GetWindowThreadProcessId(h, out p);
            var name = new StringBuilder(64);
            GetClassNameW(h, name, name.Capacity);
            if (p == pid && IsWindowVisible(h) && name.ToString() == "HoploDexPassphraseDialog") { dialog = h; return false; }
            return true;
        }, IntPtr.Zero);
        if (dialog == IntPtr.Zero) throw new Exception("no passphrase dialog for process " + pid);
        return dialog;
    }

    /// The focused control of the process's passphrase dialog.
    static IntPtr Focus(uint pid) {
        IntPtr dialog = Dialog(pid);
        uint ignored;
        var info = new GUITHREADINFO { cbSize = Marshal.SizeOf(typeof(GUITHREADINFO)) };
        uint thread = GetWindowThreadProcessId(dialog, out ignored);
        if (GetGUIThreadInfo(thread, ref info) && info.focus != IntPtr.Zero) return info.focus;
        // Another worker's app has the activation: the control the dialog
        // says has its focus (it starts a key sent to it from there). The
        // foreground isn't taken back: a key pressed for that would reach the
        // other app.
        IntPtr remembered = GetPropW(dialog, "HoploDexFocus");
        if (remembered == IntPtr.Zero) throw new Exception("the passphrase dialog has no focused control");
        return remembered;
    }

    public static void Type(uint pid, string text) {
        foreach (char c in text) {
            PostMessageW(Focus(pid), WM_CHAR, (IntPtr)c, (IntPtr)1);
            Thread.Sleep(15);
        }
    }

    public static void Press(uint pid, int vk) {
        IntPtr target = Focus(pid);
        PostMessageW(target, WM_KEYDOWN, (IntPtr)vk, (IntPtr)1);
        Thread.Sleep(30);
        PostMessageW(target, WM_KEYUP, (IntPtr)vk, (IntPtr)unchecked((int)0xC0000001));
        Thread.Sleep(30);
    }
}
'@

$codes = @{ Return = 0x0D; Escape = 0x1B; Tab = 0x09; space = 0x20; BackSpace = 0x08 }
if ($Shot) { [DialogInput]::Shot($ProcessId, $Shot) }
if ($Text) { [DialogInput]::Type($ProcessId, $Text) }
if ($Key) {
    if (-not $codes.ContainsKey($Key)) { throw "unknown key $Key" }
    [DialogInput]::Press($ProcessId, $codes[$Key])
}
