<#
Checks the installer hooks' wait for program files that are still in use (src-tauri/windows/hooks.nsh,
MARSHELL_REMOVE_HELD_PROGRAMS; issue #212), without installing anything.

It builds a small program from hooks-check.nsi that runs that macro against a folder, fills new folders under
the temp folder with empty stand-ins for Marshell's program files, and holds one of them open the way a
program that is still ending holds its own file. Nothing here starts, stops or uninstalls Marshell.

It needs the NSIS that Tauri downloads the first time an installer is built:

    pnpm tauri build --bundles nsis --config src-tauri/tauri.release.conf.json
    ./scripts/release/hooks-check.ps1

No workflow runs it: run it by hand after a change to the hooks file.
#>
[CmdletBinding()]
param(
    # makensis.exe. Without it: the one Tauri downloaded, then one on PATH, then NSIS's own install folder.
    [string]$Makensis = ''
)

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version 3.0

# The check's own limit, far below the uninstaller's 20 s, so "gives up" is seen in seconds.
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
$Exe = Join-Path $Scratch 'hooks-check.exe'
$Hooks = (Resolve-Path -LiteralPath (Join-Path $PSScriptRoot '..\..\src-tauri\windows\hooks.nsh')).Path
$built = & $Makensis /V2 "/DHOOKS=$Hooks" "/DOUT=$Exe" "/DMARSHELL_HELD_WAIT_MS=$LimitMs" (Join-Path $PSScriptRoot 'hooks-check.nsi') 2>&1
if ($LASTEXITCODE -ne 0 -or -not (Test-Path -LiteralPath $Exe -PathType Leaf)) {
    throw "makensis failed ($LASTEXITCODE): $($built -join "`n")"
}

# A new folder holding the stand-ins and $Extra. Each case gets its own.
function New-Folder([string]$Name, [string[]]$Extra = @()) {
    $dir = Join-Path $Scratch $Name
    $null = New-Item -ItemType Directory -Path $dir
    foreach ($f in $Programs + $Extra) { [System.IO.File]::WriteAllText((Join-Path $dir $f), 'stand-in') }
    return $dir
}

# Opens a file the way a running program holds its own: others may read it, nobody may delete or write it.
function Hold([string]$Path) {
    return [System.IO.File]::Open($Path, [System.IO.FileMode]::Open, [System.IO.FileAccess]::Read, [System.IO.FileShare]::Read)
}

# Runs the macro against $Dir. $ReleaseAfterMs closes $Held that long after the start; without it $Held stays open
# until the program has ended. Gives the exit code ($null when it was still running after 30 s) and the time it took.
function Invoke-Macro([string]$Dir, $Held = $null, [int]$ReleaseAfterMs = -1) {
    $sw = [System.Diagnostics.Stopwatch]::StartNew()
    $p = Start-Process -FilePath $Exe -ArgumentList "/S /D=$Dir" -PassThru
    try {
        if ($Held -and $ReleaseAfterMs -ge 0) {
            Start-Sleep -Milliseconds $ReleaseAfterMs
            $Held.Close()
        }
        $ended = $p.WaitForExit(30000)
        $ms = [int]$sw.Elapsed.TotalMilliseconds
        if (-not $ended) {
            Stop-Process -Id $p.Id -Force -ErrorAction SilentlyContinue
            return [pscustomobject]@{ Code = $null; Ms = $ms }
        }
        return [pscustomobject]@{ Code = $p.ExitCode; Ms = $ms }
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

# 1: nothing is in use.
$dir = New-Folder 'free'
$r = Invoke-Macro $dir
Check 'nothing in use: it exits 0' { if ($r.Code -eq 0) { $true } else { "exit code '$($r.Code)'" } }
Check 'nothing in use: the programs and the folder are gone' { $l = Get-Left $dir; if ($l -eq 'no folder') { $true } else { "left $l" } }
Check 'nothing in use: it does not wait' { if ($r.Ms -lt 2000) { $true } else { "took $($r.Ms) ms" } }

# 2: a helper that ends a moment after the app, which is what issue #212 is.
$dir = New-Folder 'ending'
$r = Invoke-Macro $dir (Hold (Join-Path $dir 'OpenConsole.exe')) 1200
Check 'a program that ends after 1.2 s: it exits 0' { if ($r.Code -eq 0) { $true } else { "exit code '$($r.Code)'" } }
Check 'a program that ends after 1.2 s: the programs and the folder are gone' { $l = Get-Left $dir; if ($l -eq 'no folder') { $true } else { "left $l" } }
Check "a program that ends after 1.2 s: it stops waiting then, well before the limit ($LimitMs ms)" { if ($r.Ms -lt ($LimitMs - 500)) { $true } else { "took $($r.Ms) ms" } }

# 3: a program that never ends. The uninstaller must give up, not hang.
$dir = New-Folder 'stuck'
$r = Invoke-Macro $dir (Hold (Join-Path $dir 'marshell-host.exe'))
Check "a program that never ends: it exits 0" { if ($r.Code -eq 0) { $true } else { "exit code '$($r.Code)'" } }
Check "a program that never ends: it gives up after the limit ($LimitMs ms)" {
    if ($r.Ms -ge $LimitMs -and $r.Ms -lt ($LimitMs + 5000)) { $true } else { "took $($r.Ms) ms" }
}
Check 'a program that never ends: only its file is left' { $l = Get-Left $dir; if ($l -eq 'marshell-host.exe') { $true } else { "left $l" } }

# 4: files that aren't Marshell's programs. An uninstall run in place has its own uninstall.exe in the folder, in use.
$dir = New-Folder 'others' @('uninstall.exe', 'notes.txt')
$r = Invoke-Macro $dir (Hold (Join-Path $dir 'uninstall.exe'))
Check "files that aren't Marshell's programs: they stay" { $l = Get-Left $dir; if ($l -eq 'notes.txt, uninstall.exe') { $true } else { "left $l" } }
Check "files that aren't Marshell's programs: it does not wait for them" { if ($r.Ms -lt 2000) { $true } else { "took $($r.Ms) ms" } }

# What this made, by name; a folder goes only when it is empty.
foreach ($case in 'free', 'ending', 'stuck', 'others') {
    $d = Join-Path $Scratch $case
    foreach ($f in $Programs + @('uninstall.exe', 'notes.txt')) { Remove-Item -LiteralPath (Join-Path $d $f) -Force -ErrorAction SilentlyContinue }
    if ((Test-Path -LiteralPath $d) -and -not (Get-ChildItem -LiteralPath $d -Force)) { [System.IO.Directory]::Delete($d, $false) }
}
Remove-Item -LiteralPath $Exe -Force -ErrorAction SilentlyContinue
if (-not (Get-ChildItem -LiteralPath $Scratch -Force)) { [System.IO.Directory]::Delete($Scratch, $false) }
else { Write-Host "NOTE left in ${Scratch}: $(Get-Left $Scratch)" }

Write-Host "RESULT $($script:Pass) passed, $($script:Fail) failed"
if ($script:Fail -gt 0) { exit 1 }
