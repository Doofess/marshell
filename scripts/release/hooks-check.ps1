<#
Checks the installer hooks' two waits for program files that are still in use (src-tauri/windows/hooks.nsh):
the uninstaller's MARSHELL_REMOVE_HELD_PROGRAMS (issue #212) and the installer's
MARSHELL_WAIT_FOR_HELD_PROGRAMS (issue #214), without installing anything.

It builds two small programs from hooks-check.nsi, one for each wait, that run the macro against a folder. It
fills new folders under the temp folder with stand-ins for Marshell's program files, and holds one of them
open the way a program that is still ending holds its own file. The install program stands in for the
installer's `File` lines by writing "replaced" into each program file that is there. Nothing here starts,
stops or uninstalls Marshell.

It needs the NSIS that Tauri downloads the first time an installer is built:

    pnpm tauri build --bundles nsis --config src-tauri/tauri.release.conf.json
    ./scripts/release/hooks-check.ps1

The release build runs it before the smoke test (.github/workflows/release.yml); run it by hand after a change
to the hooks file.
#>
[CmdletBinding()]
param(
    # makensis.exe. Without it: the one Tauri downloaded, then one on PATH, then NSIS's own install folder.
    [string]$Makensis = ''
)

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version 3.0

# The check's own limit, far below the real 20 s, so "gives up" is seen in seconds.
$LimitMs = 3000
$Programs = @('marshell-app.exe', 'marshell.exe', 'marshell-hook.exe', 'marshell-host.exe', 'OpenConsole.exe', 'conpty.dll')

$script:Pass = 0
$script:Fail = 0
function Check([string]$Name, [scriptblock]$Test) {
    $r = try { & $Test } catch { "threw: $($_.Exception.Message)" }
    if ($r -eq $true) { $script:Pass++; Write-Host "PASS $Name" }
    else { $script:Fail++; Write-Host "FAIL ${Name}: $r" }
}

if (-not $Makensis) {
    $onPath = Get-Command makensis.exe -CommandType Application -ErrorAction SilentlyContinue | Select-Object -First 1
    $places = @(
        (Join-Path $env:LOCALAPPDATA 'tauri\NSIS\Bin\makensis.exe'),
        (Join-Path $env:LOCALAPPDATA 'tauri\NSIS\makensis.exe'),
        $(if ($onPath) { $onPath.Source }),
        (Join-Path ${env:ProgramFiles(x86)} 'NSIS\makensis.exe')
    )
    $Makensis = $places | Where-Object { $_ -and (Test-Path -LiteralPath $_ -PathType Leaf) } | Select-Object -First 1
    if (-not $Makensis) { throw 'No makensis.exe found. Build the installer once, or pass -Makensis.' }
} elseif (-not (Test-Path -LiteralPath $Makensis -PathType Leaf)) {
    throw "makensis.exe is not at '$Makensis'."
}
Write-Host "NOTE makensis: $Makensis"

$Scratch = Join-Path ([System.IO.Path]::GetTempPath()) ('marshell-hooks-check-' + [guid]::NewGuid().ToString('N'))
$null = New-Item -ItemType Directory -Path $Scratch
$Hooks = (Resolve-Path -LiteralPath (Join-Path $PSScriptRoot '..\..\src-tauri\windows\hooks.nsh')).Path

# One program for each wait: 'uninstall' or 'install' (hooks-check.nsi's CHECK).
function Build-Check([string]$Mode) {
    $exe = Join-Path $Scratch "hooks-check-$Mode.exe"
    $built = & $Makensis /V2 "/DCHECK=$Mode" "/DHOOKS=$Hooks" "/DOUT=$exe" "/DMARSHELL_HELD_WAIT_MS=$LimitMs" (Join-Path $PSScriptRoot 'hooks-check.nsi') 2>&1
    if ($LASTEXITCODE -ne 0 -or -not (Test-Path -LiteralPath $exe -PathType Leaf)) {
        throw "makensis failed for the $Mode check ($LASTEXITCODE): $($built -join "`n")"
    }
    return $exe
}
$UninstallExe = Build-Check 'uninstall'
$InstallExe = Build-Check 'install'

# A new folder holding the stand-ins (unless $NoPrograms) and $Extra. Each case gets its own.
function New-Folder([string]$Name, [string[]]$Extra = @(), [switch]$NoPrograms) {
    $dir = Join-Path $Scratch $Name
    $null = New-Item -ItemType Directory -Path $dir
    $files = if ($NoPrograms) { $Extra } else { $Programs + $Extra }
    foreach ($f in $files) { [System.IO.File]::WriteAllText((Join-Path $dir $f), 'stand-in') }
    return $dir
}

# Opens a file the way a running program holds its own: others may read it, nobody may delete or write it.
function Hold([string]$Path) {
    return [System.IO.File]::Open($Path, [System.IO.FileMode]::Open, [System.IO.FileAccess]::Read, [System.IO.FileShare]::Read)
}

# Loads a library into this process, which maps its file as a running program's image is mapped. Close frees it,
# and only once.
function Hold-Library([string]$Path) {
    $held = [pscustomobject]@{ Handle = [System.Runtime.InteropServices.NativeLibrary]::Load($Path); Freed = $false }
    $held | Add-Member -MemberType ScriptMethod -Name Close -Value {
        if (-not $this.Freed) {
            $this.Freed = $true
            [System.Runtime.InteropServices.NativeLibrary]::Free($this.Handle)
        }
    }
    return $held
}

# Runs $Exe against $Dir. $ReleaseAfterMs closes $Held that long after the start; without it $Held stays open
# until the program has ended. The program's standard output goes to <case>.stdout.txt in the scratch folder, never
# into $Dir: the program holds that file while it runs, and the folder could not be removed with it inside.
# Gives the exit code ($null when it was still running after 30 s), the time it took and what it wrote.
function Invoke-Macro([string]$Exe, [string]$Case, [string]$Dir, $Held = $null, [int]$ReleaseAfterMs = -1) {
    $stdout = Join-Path $Scratch "$Case.stdout.txt"
    $sw = [System.Diagnostics.Stopwatch]::StartNew()
    $p = Start-Process -FilePath $Exe -ArgumentList "/S /D=$Dir" -RedirectStandardOutput $stdout -PassThru
    $null = $p.Handle
    try {
        if ($Held -and $ReleaseAfterMs -ge 0) {
            Start-Sleep -Milliseconds $ReleaseAfterMs
            $Held.Close()
        }
        $ended = $p.WaitForExit(30000)
        $ms = [int]$sw.Elapsed.TotalMilliseconds
        if (-not $ended) {
            Stop-Process -Id $p.Id -Force -ErrorAction SilentlyContinue
            return [pscustomobject]@{ Code = $null; Ms = $ms; Out = '' }
        }
        $out = if (Test-Path -LiteralPath $stdout) { ([System.IO.File]::ReadAllText($stdout)).Trim() } else { '' }
        return [pscustomobject]@{ Code = $p.ExitCode; Ms = $ms; Out = $out }
    } finally {
        if ($Held) { $Held.Close() }
    }
}

function Get-Left([string]$Dir) {
    if (-not (Test-Path -LiteralPath $Dir)) { return 'no folder' }
    $names = @(Get-ChildItem -LiteralPath $Dir -Force | ForEach-Object { $_.Name } | Sort-Object)
    if ($names.Count -eq 0) { return 'an empty folder' }
    return ($names -join ', ')
}

# The programs in $Dir that don't read "replaced", for a failure text; $null when they all do.
function Get-NotReplaced([string]$Dir) {
    $not = @($Programs | Where-Object {
        $f = Join-Path $Dir $_
        -not (Test-Path -LiteralPath $f) -or [System.IO.File]::ReadAllText($f) -ne 'replaced'
    })
    if ($not.Count -eq 0) { return $null }
    return ($not -join ', ')
}

# The programs in $Dir that no longer read "stand-in"; $null when none changed.
function Get-Changed([string]$Dir) {
    $changed = @($Programs | Where-Object {
        $f = Join-Path $Dir $_
        (Test-Path -LiteralPath $f) -and [System.IO.File]::ReadAllText($f) -ne 'stand-in'
    })
    if ($changed.Count -eq 0) { return $null }
    return ($changed -join ', ')
}

# Each program starts once on an empty folder before the timed cases. The first start of a program that was just
# built can take seconds on a CI runner (once 7.7 s, more than the uninstall wait's whole 3 s limit), and that
# time is not the wait's; the limits the cases check stay as they are.
foreach ($w in @(@('warm-uninstall', $UninstallExe), @('warm-install', $InstallExe))) {
    $r = Invoke-Macro $w[1] $w[0] (New-Folder $w[0] -NoPrograms)
    Write-Host "NOTE the first start of $(Split-Path -Leaf $w[1]) took $($r.Ms) ms (exit code '$($r.Code)')"
}

# The uninstaller's wait (MARSHELL_REMOVE_HELD_PROGRAMS).

# 1: nothing is in use.
$dir = New-Folder 'free'
$r = Invoke-Macro $UninstallExe 'free' $dir
Check 'nothing in use: it exits 0' { if ($r.Code -eq 0) { $true } else { "exit code '$($r.Code)'" } }
Check 'nothing in use: the programs and the folder are gone' { $l = Get-Left $dir; if ($l -eq 'no folder') { $true } else { "left $l" } }
Check 'nothing in use: it does not wait' { if ($r.Ms -lt 2000) { $true } else { "took $($r.Ms) ms" } }

# 2: a helper that ends a moment after the app, which is what issue #212 is.
$dir = New-Folder 'ending'
$r = Invoke-Macro $UninstallExe 'ending' $dir (Hold (Join-Path $dir 'OpenConsole.exe')) 1200
Check 'a program that ends after 1.2 s: it exits 0' { if ($r.Code -eq 0) { $true } else { "exit code '$($r.Code)'" } }
Check 'a program that ends after 1.2 s: the programs and the folder are gone' { $l = Get-Left $dir; if ($l -eq 'no folder') { $true } else { "left $l" } }
Check "a program that ends after 1.2 s: it stops waiting then, well before the limit ($LimitMs ms)" { if ($r.Ms -lt ($LimitMs - 500)) { $true } else { "took $($r.Ms) ms" } }

# 3: a program that never ends. The uninstaller must give up, not hang.
$dir = New-Folder 'stuck'
$r = Invoke-Macro $UninstallExe 'stuck' $dir (Hold (Join-Path $dir 'marshell-host.exe'))
Check "a program that never ends: it exits 0" { if ($r.Code -eq 0) { $true } else { "exit code '$($r.Code)'" } }
Check "a program that never ends: it gives up after the limit ($LimitMs ms)" {
    if ($r.Ms -ge $LimitMs -and $r.Ms -lt ($LimitMs + 5000)) { $true } else { "took $($r.Ms) ms" }
}
Check 'a program that never ends: only its file is left' { $l = Get-Left $dir; if ($l -eq 'marshell-host.exe') { $true } else { "left $l" } }

# 4: files that aren't Marshell's programs. An uninstall run in place has its own uninstall.exe in the folder, in use.
$dir = New-Folder 'others' @('uninstall.exe', 'notes.txt')
$r = Invoke-Macro $UninstallExe 'others' $dir (Hold (Join-Path $dir 'uninstall.exe'))
Check "files that aren't Marshell's programs: they stay" { $l = Get-Left $dir; if ($l -eq 'notes.txt, uninstall.exe') { $true } else { "left $l" } }
Check "files that aren't Marshell's programs: it does not wait for them" { if ($r.Ms -lt 2000) { $true } else { "took $($r.Ms) ms" } }

# The installer's wait (MARSHELL_WAIT_FOR_HELD_PROGRAMS), then the stand-in for the `File` lines.

# 5: nothing is in use.
$dir = New-Folder 'i-free'
$r = Invoke-Macro $InstallExe 'i-free' $dir
Check 'install, nothing in use: it exits 0' { if ($r.Code -eq 0) { $true } else { "exit code '$($r.Code)'" } }
Check 'install, nothing in use: every program reads replaced' { $n = Get-NotReplaced $dir; if (-not $n) { $true } else { "not replaced: $n" } }
Check 'install, nothing in use: it does not wait' { if ($r.Ms -lt 2000) { $true } else { "took $($r.Ms) ms" } }

# 6: a helper that ends a moment after the app, which is what issue #214 is.
$dir = New-Folder 'i-ending'
$r = Invoke-Macro $InstallExe 'i-ending' $dir (Hold (Join-Path $dir 'OpenConsole.exe')) 1200
Check 'install, a program that ends after 1.2 s: it exits 0' { if ($r.Code -eq 0) { $true } else { "exit code '$($r.Code)'" } }
Check 'install, a program that ends after 1.2 s: every program reads replaced' { $n = Get-NotReplaced $dir; if (-not $n) { $true } else { "not replaced: $n" } }
Check 'install, a program that ends after 1.2 s: it waited for it, and stopped waiting then' {
    if ($r.Ms -ge 1200 -and $r.Ms -lt 2500) { $true } else { "took $($r.Ms) ms" }
}

# 7: a program that never ends. The install must stop before it changes anything, and say why.
$dir = New-Folder 'i-stuck'
$r = Invoke-Macro $InstallExe 'i-stuck' $dir (Hold (Join-Path $dir 'marshell-host.exe'))
Check 'install, a program that never ends: it exits 32' { if ($r.Code -eq 32) { $true } else { "exit code '$($r.Code)'" } }
Check "install, a program that never ends: it gives up after the limit ($LimitMs ms)" {
    if ($r.Ms -ge $LimitMs -and $r.Ms -lt 8000) { $true } else { "took $($r.Ms) ms" }
}
Check 'install, a program that never ends: no program was changed' { $c = Get-Changed $dir; if (-not $c) { $true } else { "changed: $c" } }
Check 'install, a program that never ends: it says which program, and that nothing was changed' {
    $want = "Marshell wasn't updated: marshell-host.exe was still running 3 seconds after Marshell closed. Nothing was changed. Run the update again."
    if ($r.Out -ceq $want) { $true } else { "wrote '$($r.Out)'" }
}

# 8: the earlier version is already removed (a windowed upgrade's "uninstall before installing" does that): no marshell-app.exe.
$dir = New-Folder 'i-removed'
Remove-Item -LiteralPath (Join-Path $dir 'marshell-app.exe') -Force
$r = Invoke-Macro $InstallExe 'i-removed' $dir (Hold (Join-Path $dir 'conpty.dll'))
Check 'install, the earlier version already removed: it exits 32' { if ($r.Code -eq 32) { $true } else { "exit code '$($r.Code)'" } }
Check 'install, the earlier version already removed: it says so' {
    $want = "Marshell wasn't installed: conpty.dll was still running 3 seconds after Marshell closed. The earlier version had already been removed. Run the installer again."
    if ($r.Out -ceq $want) { $true } else { "wrote '$($r.Out)'" }
}

# 9: a library loaded into a running program, which maps its file as an image. Nothing else here tries that.
$dir = New-Folder 'i-loaded'
Copy-Item -LiteralPath (Join-Path $env:SystemRoot 'System32\version.dll') -Destination (Join-Path $dir 'conpty.dll') -Force
$lib = Hold-Library (Join-Path $dir 'conpty.dll')
try {
    $r = Invoke-Macro $InstallExe 'i-loaded' $dir $lib
} finally {
    $lib.Close()
}
Check 'install, a loaded DLL: it exits 32' { if ($r.Code -eq 32) { $true } else { "exit code '$($r.Code)'" } }
Check 'install, a loaded DLL: it names conpty.dll' { if ($r.Out -like '*conpty.dll*') { $true } else { "wrote '$($r.Out)'" } }

# 10: a read-only program is not in use: `File` replaces it.
$dir = New-Folder 'i-readonly'
(Get-Item -LiteralPath (Join-Path $dir 'conpty.dll')).IsReadOnly = $true
$r = Invoke-Macro $InstallExe 'i-readonly' $dir
Check 'install, a read-only program: it exits 0' { if ($r.Code -eq 0) { $true } else { "exit code '$($r.Code)'" } }
Check 'install, a read-only program: it does not wait' { if ($r.Ms -lt 2000) { $true } else { "took $($r.Ms) ms" } }
Check 'install, a read-only program: every program reads replaced' { $n = Get-NotReplaced $dir; if (-not $n) { $true } else { "not replaced: $n" } }

# 11: a read-only program that is in use. Opening it for writing fails with 5 before Windows looks at sharing, so
# the wait has to look past the flag, and has to put the flag back when it stops.
$dir = New-Folder 'i-ro-held'
(Get-Item -LiteralPath (Join-Path $dir 'conpty.dll')).IsReadOnly = $true
$r = Invoke-Macro $InstallExe 'i-ro-held' $dir (Hold (Join-Path $dir 'conpty.dll'))
Check 'install, a read-only program that is held: it exits 32' { if ($r.Code -eq 32) { $true } else { "exit code '$($r.Code)'" } }
Check 'install, a read-only program that is held: no program was changed' { $c = Get-Changed $dir; if (-not $c) { $true } else { "changed: $c" } }
Check 'install, a read-only program that is held: it says which program, and that nothing was changed' {
    $want = "Marshell wasn't updated: conpty.dll was still running 3 seconds after Marshell closed. Nothing was changed. Run the update again."
    if ($r.Out -ceq $want) { $true } else { "wrote '$($r.Out)'" }
}
Check 'install, a read-only program that is held: it is still read-only' {
    if ((Get-Item -LiteralPath (Join-Path $dir 'conpty.dll')).IsReadOnly) { $true } else { 'the flag was lost' }
}

# 12: a read-only library loaded into a running program.
$dir = New-Folder 'i-ro-loaded'
Copy-Item -LiteralPath (Join-Path $env:SystemRoot 'System32\version.dll') -Destination (Join-Path $dir 'conpty.dll') -Force
(Get-Item -LiteralPath (Join-Path $dir 'conpty.dll')).IsReadOnly = $true
$lib = Hold-Library (Join-Path $dir 'conpty.dll')
try {
    $r = Invoke-Macro $InstallExe 'i-ro-loaded' $dir $lib
} finally {
    $lib.Close()
}
Check 'install, a read-only loaded DLL: it exits 32' { if ($r.Code -eq 32) { $true } else { "exit code '$($r.Code)'" } }
Check 'install, a read-only loaded DLL: it names conpty.dll' { if ($r.Out -like '*conpty.dll*') { $true } else { "wrote '$($r.Out)'" } }
Check 'install, a read-only loaded DLL: it is still read-only' {
    if ((Get-Item -LiteralPath (Join-Path $dir 'conpty.dll')).IsReadOnly) { $true } else { 'the flag was lost' }
}

# 13: a program held for writing by something that shares writing. `File` opens with sharing for reading only,
# so it fails on this file, and the wait has to see that.
$dir = New-Folder 'i-share-write'
$writer = [System.IO.File]::Open((Join-Path $dir 'OpenConsole.exe'), [System.IO.FileMode]::Open, [System.IO.FileAccess]::ReadWrite, [System.IO.FileShare]::ReadWrite)
$r = Invoke-Macro $InstallExe 'i-share-write' $dir $writer
Check 'install, a program held for writing that shares writing: it exits 32' { if ($r.Code -eq 32) { $true } else { "exit code '$($r.Code)'" } }
Check 'install, a program held for writing that shares writing: it names OpenConsole.exe' { if ($r.Out -like '*OpenConsole.exe*') { $true } else { "wrote '$($r.Out)'" } }

# 14: a first install: there is no program to wait for, and a file that isn't Marshell's is not waited for.
$dir = New-Folder 'i-none' @('uninstall.exe', 'notes.txt') -NoPrograms
$r = Invoke-Macro $InstallExe 'i-none' $dir (Hold (Join-Path $dir 'uninstall.exe'))
Check 'install, no programs yet: it exits 0' { if ($r.Code -eq 0) { $true } else { "exit code '$($r.Code)'" } }
Check 'install, no programs yet: it does not wait' { if ($r.Ms -lt 2000) { $true } else { "took $($r.Ms) ms" } }
Check 'install, no programs yet: the folder holds just those two' { $l = Get-Left $dir; if ($l -eq 'notes.txt, uninstall.exe') { $true } else { "left $l" } }

# What this made, by name; a folder goes only when it is empty.
foreach ($case in 'warm-uninstall', 'warm-install', 'free', 'ending', 'stuck', 'others', 'i-free', 'i-ending', 'i-stuck', 'i-removed', 'i-loaded', 'i-readonly', 'i-ro-held', 'i-ro-loaded', 'i-share-write', 'i-none') {
    $d = Join-Path $Scratch $case
    foreach ($f in $Programs + @('uninstall.exe', 'notes.txt')) { Remove-Item -LiteralPath (Join-Path $d $f) -Force -ErrorAction SilentlyContinue }
    if ((Test-Path -LiteralPath $d) -and -not (Get-ChildItem -LiteralPath $d -Force)) { [System.IO.Directory]::Delete($d, $false) }
    Remove-Item -LiteralPath (Join-Path $Scratch "$case.stdout.txt") -Force -ErrorAction SilentlyContinue
}
Remove-Item -LiteralPath $UninstallExe, $InstallExe -Force -ErrorAction SilentlyContinue
if (-not (Get-ChildItem -LiteralPath $Scratch -Force)) { [System.IO.Directory]::Delete($Scratch, $false) }
else { Write-Host "NOTE left in ${Scratch}: $(Get-Left $Scratch)" }

Write-Host "RESULT $($script:Pass) passed, $($script:Fail) failed"
if ($script:Fail -gt 0) { exit 1 }
