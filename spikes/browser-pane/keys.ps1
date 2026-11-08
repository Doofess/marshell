# Spike 6q-0: real key input for the browser spike (6q-0 decision 2). SendInput only, never the mouse. Before every key it
# checks that the foreground window belongs to the dev app's process and has a non-empty rect, and aborts otherwise.
#
# Count:  pwsh -NoProfile -File keys.ps1 -AppPid <pid> -Log <home>\spike-browser.jsonl -Keys "Ctrl+Shift+K","Escape" -Reps 20
# One:    pwsh -NoProfile -File keys.ps1 -AppPid <pid> -Key "F5"      |  -Type "abc"  |  -Key "Ctrl+Shift+K" -HoldMs 1000
param(
    [Parameter(Mandatory)][int]$AppPid,
    [string]$Log,
    [string[]]$Keys,
    [int]$Reps = 20,
    [string]$Key,
    [string]$Type,
    [int]$HoldMs = 0,
    [int]$WaitForegroundSec = 0,
    # Counting: when another window takes the front, wait this long for the dev window to come back, run -Refocus, and
    # send the press again. 0 stops at the first refusal.
    [int]$ResumeSec = 0,
    [scriptblock]$Refocus
)
$ErrorActionPreference = 'Stop'
Add-Type -TypeDefinition @'
using System;
using System.Runtime.InteropServices;
public static class SpikeKeys {
  [StructLayout(LayoutKind.Sequential)] public struct KEYBDINPUT { public ushort wVk; public ushort wScan; public uint dwFlags; public uint time; public IntPtr dwExtraInfo; }
  [StructLayout(LayoutKind.Sequential)] public struct MOUSEINPUT { public int dx, dy; public uint mouseData, dwFlags, time; public IntPtr dwExtraInfo; }
  [StructLayout(LayoutKind.Explicit)] public struct UNION { [FieldOffset(0)] public KEYBDINPUT ki; [FieldOffset(0)] public MOUSEINPUT mi; }
  [StructLayout(LayoutKind.Sequential)] public struct INPUT { public uint type; public UNION u; }
  [StructLayout(LayoutKind.Sequential)] public struct RECT { public int L, T, R, B; }
  [DllImport("user32.dll")] public static extern uint SendInput(uint n, INPUT[] inputs, int size);
  [DllImport("user32.dll")] public static extern uint MapVirtualKeyW(uint code, uint type);
  [DllImport("user32.dll")] public static extern IntPtr GetForegroundWindow();
  [DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr h, out uint pid);
  [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr h, out RECT r);
  [StructLayout(LayoutKind.Sequential)] public struct POINT { public int X, Y; }
  [DllImport("user32.dll")] public static extern bool GetCursorPos(out POINT p);
  public const uint KEYUP = 2, UNICODE = 4;
  public static INPUT Vk(ushort vk, bool up) {
    var i = new INPUT { type = 1 };
    i.u.ki.wVk = vk; i.u.ki.wScan = (ushort)MapVirtualKeyW(vk, 0); i.u.ki.dwFlags = up ? KEYUP : 0;
    return i;
  }
  public static INPUT Uni(char c, bool up) {
    var i = new INPUT { type = 1 };
    i.u.ki.wScan = c; i.u.ki.dwFlags = UNICODE | (up ? KEYUP : 0);
    return i;
  }
  public static uint Send(INPUT[] xs) { return SendInput((uint)xs.Length, xs, Marshal.SizeOf(typeof(INPUT))); }
}
'@

function Assert-Foreground {
    $h = [SpikeKeys]::GetForegroundWindow()
    $fgPid = 0
    [void][SpikeKeys]::GetWindowThreadProcessId($h, [ref]$fgPid)
    $r = New-Object SpikeKeys+RECT
    [void][SpikeKeys]::GetWindowRect($h, [ref]$r)
    if ($fgPid -ne $AppPid -or ($r.R - $r.L) -le 0 -or ($r.B - $r.T) -le 0) {
        throw "ABORT: the foreground window belongs to pid $fgPid, not the dev app ($AppPid); nothing sent"
    }
}

$VK = @{ Ctrl = 0x11; Shift = 0x10; Alt = 0x12; Tab = 0x09; Escape = 0x1B; Enter = 0x0D; F5 = 0x74; F12 = 0x7B }
function Parse-Chord([string]$chord) {
    $parts = $chord -split '\+'
    # A lone key has no modifiers: $parts[0..-1] would be the key twice, and the key would go down three times.
    # (An if expression would hand back $null for the empty case, and $null piped to ForEach-Object sends a key 0.)
    $mods = @()
    if ($parts.Count -gt 1) { $mods = @($parts[0..($parts.Count - 2)] | Where-Object { $_ } | ForEach-Object { $VK[$_] }) }
    $last = $parts[-1]
    $k = if ($VK.ContainsKey($last)) { $VK[$last] } elseif ($last.Length -eq 1) { [int][char]$last.ToUpper() } else { throw "unknown key $last" }
    $code = if ($last.Length -eq 1 -and $last -match '[A-Za-z]') { 'Key' + $last.ToUpper() } elseif ($last.Length -eq 1) { 'Digit' + $last } else { $last }
    [pscustomobject]@{ Mods = $mods; Key = $k; Code = $code }
}

function Send-Chord($c, [int]$holdMs = 0) {
    Assert-Foreground
    $down = @($c.Mods | ForEach-Object { [SpikeKeys]::Vk([uint16]$_, $false) }) + [SpikeKeys]::Vk([uint16]$c.Key, $false)
    $rev = @($c.Mods); [array]::Reverse($rev)
    $up = @([SpikeKeys]::Vk([uint16]$c.Key, $true)) + @($rev | ForEach-Object { [SpikeKeys]::Vk([uint16]$_, $true) })
    [void][SpikeKeys]::Send([SpikeKeys+INPUT[]]$down)
    # The key-ups go out even if the hold is interrupted, so no key or modifier is left down.
    try {
        # SendInput doesn't repeat a held key; the keyboard's repeat is more key-downs with the key already down.
        for ($t = 500; $t -lt $holdMs; $t += 33) {
            if ($t -eq 500) { Start-Sleep -Milliseconds 500 } else { Start-Sleep -Milliseconds 33 }
            [void][SpikeKeys]::Send([SpikeKeys+INPUT[]]@([SpikeKeys]::Vk([uint16]$c.Key, $false)))
        }
    } finally {
        [void][SpikeKeys]::Send([SpikeKeys+INPUT[]]$up)
    }
}

function Send-Text([string]$text) {
    foreach ($ch in $text.ToCharArray()) {
        Assert-Foreground
        [void][SpikeKeys]::Send([SpikeKeys+INPUT[]]@([SpikeKeys]::Uni($ch, $false), [SpikeKeys]::Uni($ch, $true)))
        Start-Sleep -Milliseconds 30
    }
}

function Read-LogFrom([int]$from) {
    $lines = @(Get-Content -LiteralPath $Log)
    if ($lines.Count -le $from) { return @() }
    # The app may be halfway through writing the last line; skip it, the next read sees it whole.
    @($lines[$from..($lines.Count - 1)] | ForEach-Object { try { $_ | ConvertFrom-Json } catch { } })
}

function Wait-For([int]$from, [scriptblock]$match, [int]$ms) {
    $deadline = (Get-Date).AddMilliseconds($ms)
    while ((Get-Date) -lt $deadline) {
        $hit = Read-LogFrom $from | Where-Object $match | Select-Object -First 1
        if ($hit) { return $hit }
        Start-Sleep -Milliseconds 50
    }
    $null
}

# The dev window must stay in front for $stableMs on end, so a click that passes through it doesn't count.
function Wait-Foreground([int]$sec, [int]$stableMs = 2000) {
    $deadline = (Get-Date).AddSeconds($sec)
    $since = $null
    while ((Get-Date) -lt $deadline) {
        $ok = $true
        try { Assert-Foreground } catch { $ok = $false }
        if (-not $ok) { $since = $null }
        elseif (-not $since) { $since = Get-Date }
        elseif (((Get-Date) - $since).TotalMilliseconds -ge $stableMs) { return $true }
        Start-Sleep -Milliseconds 100
    }
    $false
}

if ($WaitForegroundSec) {
    if (Wait-Foreground $WaitForegroundSec) { "the dev window is in front"; return }
    throw "ABORT: the dev window didn't stay in front within $WaitForegroundSec s"
}
if ($Type) { Send-Text $Type; "typed $($Type.Length) characters"; return }
if ($Key) { Send-Chord (Parse-Chord $Key) $HoldMs; "sent $Key" + $(if ($HoldMs) { " held $HoldMs ms" } else { '' }); return }

if (-not $Log -or -not $Keys) { throw "-Log and -Keys are needed to count" }
foreach ($name in $Keys) {
    $c = Parse-Chord $name
    $tally = [ordered]@{ key = $name; sent = 0; forwarded = 0; received = 0; acted = 0; refocused = 0; notSent = 0; mouseMoved = 0 }
    $p0 = New-Object SpikeKeys+POINT
    [void][SpikeKeys]::GetCursorPos([ref]$p0)
    for ($i = 1; $i -le $Reps; $i++) {
        $from = @(Get-Content -LiteralPath $Log).Count
        try { Send-Chord $c } catch {
            $tally.notSent++
            Write-Warning $_
            if ($ResumeSec -and $tally.notSent -le 5 -and (Wait-Foreground $ResumeSec)) {
                if ($Refocus) { & $Refocus }
                Start-Sleep -Milliseconds 300
                $i--
                continue
            }
            break
        }
        $tally.sent++
        # Someone using the mouse during the run can move focus out of the page; such a press is marked, not trusted.
        $p1 = New-Object SpikeKeys+POINT
        [void][SpikeKeys]::GetCursorPos([ref]$p1)
        if ($p1.X -ne $p0.X -or $p1.Y -ne $p0.Y) { $tally.mouseMoved++; Write-Warning "the mouse moved before press $i of $name"; $p0 = $p1 }
        if (Wait-For $from { $_.by -eq 'rust' -and $_.event -eq 'key-forwarded' -and $_.code -eq $c.Code } 2000) { $tally.forwarded++ }
        if (Wait-For $from { $_.by -eq 'front' -and $_.event -eq 'key-received' -and $_.code -eq $c.Code } 2000) { $tally.received++ }
        if (Wait-For $from { $_.by -eq 'front' -and $_.event -eq 'action' } 2000) { $tally.acted++ }
        $reset = Wait-For $from { $_.by -eq 'front' -and $_.event -eq 'reset' } 3000
        if ($reset -and (Wait-For $from { $_.by -eq 'rust' -and $_.event -eq 'page-focus' -and $_.t -ge $reset.t - 5 } 3000)) { $tally.refocused++ }
        Start-Sleep -Milliseconds 150
    }
    [pscustomobject]$tally | ConvertTo-Json -Compress
}
