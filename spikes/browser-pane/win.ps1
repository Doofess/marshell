# Spike 6q-0: window switching and guarded clicks for the focus checks. Dot-source it: . spikes/browser-pane/win.ps1
#   Set-Front <hwnd>                 brings a window to the front without sending input (AttachThreadInput).
#   Send-AltTab -From <pid> -To <pid> real Alt+Tab, sent only while <From> holds the foreground; checks <To> has it after.
#   Send-Click -Root <hwnd> -X -Y    a left click at screen pixel (X, Y), sent only if the foreground window and the window
#                                    under the point both belong to <Root>'s window; the cursor goes back where it was.
#   Get-Fg                           the foreground window's handle and pid.
if (-not ('SpikeWin' -as [type])) {
    Add-Type -TypeDefinition @'
using System;
using System.Runtime.InteropServices;
public static class SpikeWin {
  [StructLayout(LayoutKind.Sequential)] public struct POINT { public int X, Y; }
  [StructLayout(LayoutKind.Sequential)] public struct RECT { public int L, T, R, B; }
  [StructLayout(LayoutKind.Sequential)] public struct KEYBDINPUT { public ushort wVk; public ushort wScan; public uint dwFlags; public uint time; public IntPtr dwExtraInfo; }
  [StructLayout(LayoutKind.Sequential)] public struct MOUSEINPUT { public int dx, dy; public uint mouseData, dwFlags, time; public IntPtr dwExtraInfo; }
  [StructLayout(LayoutKind.Explicit)] public struct UNION { [FieldOffset(0)] public KEYBDINPUT ki; [FieldOffset(0)] public MOUSEINPUT mi; }
  [StructLayout(LayoutKind.Sequential)] public struct INPUT { public uint type; public UNION u; }
  [DllImport("user32.dll")] public static extern uint SendInput(uint n, INPUT[] inputs, int size);
  [DllImport("user32.dll")] public static extern IntPtr GetForegroundWindow();
  [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr h);
  [DllImport("user32.dll")] public static extern bool BringWindowToTop(IntPtr h);
  [DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr h, out uint pid);
  [DllImport("user32.dll")] public static extern bool AttachThreadInput(uint a, uint b, bool attach);
  [DllImport("kernel32.dll")] public static extern uint GetCurrentThreadId();
  [DllImport("user32.dll")] public static extern IntPtr WindowFromPoint(POINT p);
  [DllImport("user32.dll")] public static extern IntPtr GetAncestor(IntPtr h, uint flags);
  [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr h, out RECT r);
  [DllImport("user32.dll")] public static extern bool GetCursorPos(out POINT p);
  [DllImport("user32.dll")] public static extern bool SetCursorPos(int x, int y);
  [DllImport("user32.dll")] public static extern bool ClientToScreen(IntPtr h, ref POINT p);
  [DllImport("user32.dll")] public static extern bool IsIconic(IntPtr h);
  public static INPUT Key(ushort vk, bool up) { var i = new INPUT { type = 1 }; i.u.ki.wVk = vk; i.u.ki.dwFlags = up ? 2u : 0u; return i; }
  public static INPUT Mouse(uint flags) { var i = new INPUT { type = 0 }; i.u.mi.dwFlags = flags; return i; }
  public static uint Send(INPUT[] xs) { return SendInput((uint)xs.Length, xs, Marshal.SizeOf(typeof(INPUT))); }
  public static IntPtr Root(IntPtr h) { return GetAncestor(h, 2); }
}
'@
}

function Get-Fg {
    $h = [SpikeWin]::GetForegroundWindow()
    $p = 0
    [void][SpikeWin]::GetWindowThreadProcessId($h, [ref]$p)
    [pscustomobject]@{ Hwnd = $h; Pid = [int]$p }
}

# -TapPid: when that process (the spike's own away window) holds the foreground, an Alt tap is sent to it first, which
# lets this process hand the foreground over. No key is ever sent to any other process.
function Set-Front([IntPtr]$hwnd, [int]$TapPid = 0) {
    for ($try = 1; $try -le 3; $try++) {
        $fg = [SpikeWin]::GetForegroundWindow()
        if ($fg -eq $hwnd) { return }
        $x = 0
        $fgThread = [SpikeWin]::GetWindowThreadProcessId($fg, [ref]$x)
        if ($TapPid -and $x -eq $TapPid) {
            [void][SpikeWin]::Send([SpikeWin+INPUT[]]@([SpikeWin]::Key(0x12, $false), [SpikeWin]::Key(0x12, $true)))
        }
        $me = [SpikeWin]::GetCurrentThreadId()
        [void][SpikeWin]::AttachThreadInput($me, $fgThread, $true)
        try { [void][SpikeWin]::SetForegroundWindow($hwnd); [void][SpikeWin]::BringWindowToTop($hwnd) }
        finally { [void][SpikeWin]::AttachThreadInput($me, $fgThread, $false) }
        Start-Sleep -Milliseconds 400
        if ([SpikeWin]::GetForegroundWindow() -eq $hwnd) { return }
    }
    throw "ABORT: could not bring window $hwnd to the front"
}

function Send-AltTab([int]$From, [int]$To) {
    $fg = Get-Fg
    if ($fg.Pid -ne $From) { throw "ABORT: the foreground is pid $($fg.Pid), not $From; Alt+Tab not sent" }
    $alt = 0x12; $tab = 0x09
    [void][SpikeWin]::Send([SpikeWin+INPUT[]]@([SpikeWin]::Key($alt, $false), [SpikeWin]::Key($tab, $false), [SpikeWin]::Key($tab, $true)))
    Start-Sleep -Milliseconds 150
    [void][SpikeWin]::Send([SpikeWin+INPUT[]]@([SpikeWin]::Key($alt, $true)))
    $deadline = (Get-Date).AddSeconds(3)
    while ((Get-Date) -lt $deadline) { if ((Get-Fg).Pid -eq $To) { return }; Start-Sleep -Milliseconds 100 }
    throw "ABORT: Alt+Tab from $From went to pid $((Get-Fg).Pid), not $To"
}

function Send-Click([IntPtr]$Root, [int]$X, [int]$Y, [int]$AllowFgPid = 0) {
    # -AllowFgPid: the click may also be sent while that process (the spike's own away window) holds the foreground; the
    # window under the point must still be the dev app's.
    $fg = [SpikeWin]::GetForegroundWindow()
    $ok = [SpikeWin]::Root($fg) -eq $Root -or $fg -eq $Root -or ($AllowFgPid -and (Get-Fg).Pid -eq $AllowFgPid)
    if (-not $ok) { throw "ABORT: the foreground window ($fg, root $([SpikeWin]::Root($fg)), pid $((Get-Fg).Pid)) is not the dev app's ($Root); click not sent" }
    $r = New-Object SpikeWin+RECT
    [void][SpikeWin]::GetWindowRect($Root, [ref]$r)
    if ($X -lt $r.L -or $X -ge $r.R -or $Y -lt $r.T -or $Y -ge $r.B) { throw "ABORT: ($X, $Y) is outside the dev app's window; click not sent" }
    $pt = New-Object SpikeWin+POINT
    $pt.X = $X; $pt.Y = $Y
    $under = [SpikeWin]::WindowFromPoint($pt)
    if ([SpikeWin]::Root($under) -ne $Root) { throw "ABORT: the window under ($X, $Y) is not the dev app's; click not sent" }
    $was = New-Object SpikeWin+POINT
    [void][SpikeWin]::GetCursorPos([ref]$was)
    [void][SpikeWin]::SetCursorPos($X, $Y)
    Start-Sleep -Milliseconds 60
    [void][SpikeWin]::Send([SpikeWin+INPUT[]]@([SpikeWin]::Mouse(0x0002), [SpikeWin]::Mouse(0x0004)))
    Start-Sleep -Milliseconds 120
    [void][SpikeWin]::SetCursorPos($was.X, $was.Y)
}

# Screen pixel of a point given in the main window's client coordinates (CSS px times the device pixel ratio).
function ConvertTo-Screen([IntPtr]$Root, [double]$X, [double]$Y, [double]$Ratio = 1) {
    $pt = New-Object SpikeWin+POINT
    $pt.X = [int][math]::Round($X * $Ratio); $pt.Y = [int][math]::Round($Y * $Ratio)
    [void][SpikeWin]::ClientToScreen($Root, [ref]$pt)
    $pt
}
