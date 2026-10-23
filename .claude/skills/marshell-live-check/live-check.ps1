<#
.SYNOPSIS
Starts and stops the dev Marshell app for a live check, with every home redirected to a scratch folder.

.DESCRIPTION
start: creates <Scratch>\mh (MARSHELL_HOME) and <Scratch>\cc (CLAUDE_CONFIG_DIR), starts `pnpm tauri dev`
       in this checkout with CDP on 127.0.0.1:9333, and records the root PID in <Scratch>\pids.json.
tree:  prints the process tree under the recorded root PID and stops nothing.
stop:  prints the tree and stops it by PID, leaves first. If the tree holds an agent CLI (claude.exe,
       codex.exe, node_repl.exe), nothing is stopped unless every such PID is named in -AlsoStop:
       the person may be using the dev window, and their sessions are not the check's to end.
Nothing here deletes a folder.

.EXAMPLE
pwsh -NoProfile -File live-check.ps1 start -Scratch C:\Users\me\AppData\Local\Temp\lc-5b-1
pwsh -NoProfile -File live-check.ps1 tree  -Scratch C:\Users\me\AppData\Local\Temp\lc-5b-1
pwsh -NoProfile -File live-check.ps1 stop  -Scratch C:\Users\me\AppData\Local\Temp\lc-5b-1 -AlsoStop 1234
#>
param(
    [Parameter(Mandatory, Position = 0)][ValidateSet('start', 'tree', 'stop')][string]$Mode,
    [Parameter(Mandatory)][string]$Scratch,
    [int[]]$AlsoStop = @(),
    # The checkout to run; defaults to the one holding this script.
    [string]$Repo = (Join-Path $PSScriptRoot '..\..\..'),
    [int]$Port = 9333
)
$ErrorActionPreference = 'Stop'
$AgentNames = @('claude.exe', 'codex.exe', 'node_repl.exe')

if (-not [System.IO.Path]::IsPathRooted($Scratch)) { throw "Scratch must be an absolute path: $Scratch" }
$Scratch = [System.IO.Path]::GetFullPath($Scratch).TrimEnd('\')
foreach ($real in @("$HOME\.marshell", "$HOME\.claude", "$HOME\.codex")) {
    $r = [System.IO.Path]::GetFullPath($real).TrimEnd('\')
    if ($Scratch -ieq $r -or $Scratch.StartsWith("$r\", 'OrdinalIgnoreCase')) { throw "Scratch is inside a real config folder: $real" }
}
if ((Split-Path $Scratch -Leaf) -ieq '.marshell') { throw 'Never name the scratch folder .marshell.' }
$PidsFile = Join-Path $Scratch 'pids.json'

function Get-Tree([int]$RootPid) {
    $all = Get-CimInstance Win32_Process -Property ProcessId, ParentProcessId, Name, CreationDate
    $byParent = $all | Group-Object ParentProcessId -AsHashTable -AsString
    $root = $all | Where-Object ProcessId -eq $RootPid
    if (-not $root) { return @() }
    $out = [System.Collections.Generic.List[object]]::new()
    $walk = {
        param($p, $depth)
        $out.Add([pscustomobject]@{ Depth = $depth; Name = $p.Name; PID = [int]$p.ProcessId; Parent = [int]$p.ParentProcessId })
        foreach ($c in @($byParent["$($p.ProcessId)"])) {
            # A child created before its parent is a reused parent PID, not a real child.
            if ($c -and $c.CreationDate -ge $p.CreationDate) { & $walk $c ($depth + 1) }
        }
    }
    & $walk $root 0
    return $out
}

function Read-Record {
    if (-not (Test-Path -LiteralPath $PidsFile)) { throw "No $PidsFile; was start run with this Scratch?" }
    $rec = Get-Content -LiteralPath $PidsFile -Raw | ConvertFrom-Json
    $p = Get-CimInstance Win32_Process -Filter "ProcessId = $($rec.root)" -Property Name, CreationDate
    if (-not $p) { Write-Output "Root PID $($rec.root) is no longer running."; return $null }
    # ConvertFrom-Json may already have turned `created` into a DateTime; compare as times either way.
    $recorded = ([datetime]$rec.created).ToUniversalTime()
    if ($p.Name -ne $rec.name -or [math]::Abs(($p.CreationDate.ToUniversalTime() - $recorded).TotalMilliseconds) -gt 1) {
        throw "PID $($rec.root) is now a different process ($($p.Name)); not touching it."
    }
    return $rec
}

function Show-Tree($tree) {
    $tree | ForEach-Object { '{0}{1}  pid {2}  parent {3}' -f ('  ' * $_.Depth), $_.Name, $_.PID, $_.Parent } | Write-Output
}

switch ($Mode) {
    'start' {
        if ((Test-Path -LiteralPath $Scratch) -and (Get-ChildItem -LiteralPath $Scratch -Force | Select-Object -First 1)) {
            throw "Scratch is not empty: $Scratch. Use a new literal path; never delete an old one to reuse it."
        }
        if (-not (Test-Path -LiteralPath $Scratch)) { New-Item -ItemType Directory -Path $Scratch | Out-Null }
        $mh = New-Item -ItemType Directory -Path (Join-Path $Scratch 'mh')
        $cc = New-Item -ItemType Directory -Path (Join-Path $Scratch 'cc')
        $repo = (Resolve-Path -LiteralPath $Repo).Path
        if (-not (Test-Path -LiteralPath (Join-Path $repo 'src-tauri\tauri.conf.json'))) { throw "Not a Marshell checkout: $repo" }
        $env:MARSHELL_HOME = $mh.FullName
        $env:CLAUDE_CONFIG_DIR = $cc.FullName
        $env:WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS = "--remote-debugging-port=$Port"
        $proc = Start-Process -FilePath 'cmd.exe' -ArgumentList '/d', '/c', 'pnpm tauri dev' -WorkingDirectory $repo `
            -NoNewWindow -PassThru -RedirectStandardOutput (Join-Path $Scratch 'dev.out.log') `
            -RedirectStandardError (Join-Path $Scratch 'dev.err.log')
        $created = (Get-CimInstance Win32_Process -Filter "ProcessId = $($proc.Id)" -Property CreationDate).CreationDate
        [ordered]@{
            root = $proc.Id; name = 'cmd.exe'; created = $created.ToUniversalTime().ToString('o')
            repo = $repo; marshellHome = $mh.FullName; claudeConfigDir = $cc.FullName; cdp = "http://127.0.0.1:$Port"
        } | ConvertTo-Json | Set-Content -LiteralPath $PidsFile -Encoding utf8NoBOM
        Write-Output "Started pnpm tauri dev in $repo, root PID $($proc.Id)."
        Write-Output "MARSHELL_HOME=$($mh.FullName)"
        Write-Output "CLAUDE_CONFIG_DIR=$($cc.FullName)"
        Write-Output "Logs: $Scratch\dev.out.log, $Scratch\dev.err.log. CDP: http://127.0.0.1:$Port/json/version"
    }
    'tree' {
        $rec = Read-Record
        if ($rec) { Show-Tree (Get-Tree $rec.root) }
    }
    'stop' {
        $rec = Read-Record
        if (-not $rec) { return }
        $tree = @(Get-Tree $rec.root)
        Show-Tree $tree
        $agents = @($tree | Where-Object { $AgentNames -contains $_.Name.ToLowerInvariant() })
        $unnamed = @($agents | Where-Object { $AlsoStop -notcontains $_.PID })
        if ($unnamed.Count) {
            Write-Output ''
            Write-Output 'Stopped nothing. The tree holds agent CLIs not named in -AlsoStop:'
            $unnamed | ForEach-Object { Write-Output "  $($_.Name) pid $($_.PID)" }
            Write-Output 'Name the ones this check started with -AlsoStop <pid,...>. Ask the user about any other.'
            exit 2
        }
        # Leaves first, so no parent respawns or orphans a child mid-stop.
        foreach ($p in ($tree | Sort-Object Depth -Descending)) {
            try { Stop-Process -Id $p.PID -Force -ErrorAction Stop; $state = 'stopped' }
            catch { $state = if (Get-Process -Id $p.PID -ErrorAction SilentlyContinue) { "failed: $($_.Exception.Message)" } else { 'already gone' } }
            Write-Output ("{0} pid {1}: {2}" -f $p.Name, $p.PID, $state)
        }
        Start-Sleep -Milliseconds 500
        $left = @($tree | Where-Object { Get-Process -Id $_.PID -ErrorAction SilentlyContinue })
        if ($left.Count) { Write-Output "Still running: $(($left | ForEach-Object { "$($_.Name) $($_.PID)" }) -join ', ')"; exit 1 }
        Write-Output "All $($tree.Count) recorded processes are gone. $Scratch was left in place."
    }
}
