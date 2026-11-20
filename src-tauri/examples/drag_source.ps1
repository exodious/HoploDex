# For scripts/windows/drop-check.ps1: a window of its own process holding a
# file to drag, as Explorer offers one, the counterpart of `drop_check
# --drag-source` on Linux and drag_source.swift on macOS.
#
#   drag_source.ps1 FILE X Y   a 160 by 160 pixel window, its top-left corner
#                              at X Y (screen pixels), until killed
param(
    [Parameter(Mandatory)][string]$File,
    [Parameter(Mandatory)][int]$X,
    [Parameter(Mandatory)][int]$Y
)
$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName System.Windows.Forms, System.Drawing
Add-Type @'
using System; using System.Runtime.InteropServices;
public static class Dpi {
    [DllImport("user32.dll")] public static extern bool SetProcessDpiAwarenessContext(IntPtr v);
}
'@
[Dpi]::SetProcessDpiAwarenessContext([IntPtr]::new(-4)) | Out-Null
$form = New-Object System.Windows.Forms.Form
$form.FormBorderStyle = 'None'
$form.StartPosition = 'Manual'
$form.Location = New-Object System.Drawing.Point($X, $Y)
$form.Size = New-Object System.Drawing.Size(160, 160)
$form.BackColor = [System.Drawing.Color]::Teal
$form.TopMost = $true
$form.Add_MouseDown({
    $data = New-Object System.Windows.Forms.DataObject
    $files = New-Object System.Collections.Specialized.StringCollection
    [void]$files.Add($File)
    $data.SetFileDropList($files)
    [void]$form.DoDragDrop($data, [System.Windows.Forms.DragDropEffects]::Copy)
})
[System.Windows.Forms.Application]::Run($form)
