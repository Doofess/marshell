# Spike 6q-0: captures the dev app's main window with PrintWindow (PW_RENDERFULLCONTENT), never the screen, so no other
# app's window can end up in the picture.
# pwsh -NoProfile -File spikes/browser-pane/shot.ps1 -AppPid <pid> -Out <png>
param(
    [Parameter(Mandatory)][int]$AppPid,
    [Parameter(Mandatory)][string]$Out
)
$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName System.Drawing
Add-Type -TypeDefinition @'
using System;
using System.Runtime.InteropServices;
public static class SpikeShot {
  [StructLayout(LayoutKind.Sequential)] public struct RECT { public int L, T, R, B; }
  [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr h, out RECT r);
  [DllImport("user32.dll")] public static extern bool PrintWindow(IntPtr h, IntPtr dc, uint flags);
  [DllImport("user32.dll")] public static extern IntPtr SetThreadDpiAwarenessContext(IntPtr c);
}
'@
[void][SpikeShot]::SetThreadDpiAwarenessContext([IntPtr]::new(-4))
$p = Get-Process -Id $AppPid
$h = $p.MainWindowHandle
if ($h -eq [IntPtr]::Zero) { throw "process $AppPid has no main window" }
$r = New-Object SpikeShot+RECT
[void][SpikeShot]::GetWindowRect($h, [ref]$r)
$w = $r.R - $r.L; $hgt = $r.B - $r.T
if ($w -le 0 -or $hgt -le 0) { throw "window rect is empty" }
$bmp = New-Object System.Drawing.Bitmap $w, $hgt
$g = [System.Drawing.Graphics]::FromImage($bmp)
$dc = $g.GetHdc()
$ok = [SpikeShot]::PrintWindow($h, $dc, 2)
$g.ReleaseHdc($dc); $g.Dispose()
if (-not $ok) { throw "PrintWindow failed" }
$bmp.Save($Out, [System.Drawing.Imaging.ImageFormat]::Png); $bmp.Dispose()
"{0}x{1} -> {2}" -f $w, $hgt, $Out
