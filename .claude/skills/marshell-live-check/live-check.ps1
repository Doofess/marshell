<#
.SYNOPSIS
Starts and stops the dev Marshell app for a live check, with every home redirected to a scratch folder.

.DESCRIPTION
start: creates <Scratch>\mh (MARSHELL_HOME), <Scratch>\cc (CLAUDE_CONFIG_DIR, for the Claude sessions the
       check starts and the transcripts History copies), <Scratch>\cx (CODEX_HOME, so History never reads the
       real ~/.codex) and <Scratch>\proj\.claude\settings.local.json seeded with {} (MARSHELL_CLAUDE_SETTINGS, where
       Marshell installs Claude's hooks, so onboarding's "Set up" never writes the real ~/.claude), starts
       `pnpm tauri dev` in this checkout with CDP on 127.0.0.1:9333, and records the root PID in <Scratch>\pids.json.
       -Seed <folder> first copies <folder>\cc and <folder>\cx into them, so the app finds them on its first run.
tree:  prints the process tree under the recorded root PID and stops nothing.
stop:  prints the tree and stops it by PID, leaves first. If the tree holds an agent CLI (claude.exe,
       codex.exe, node_repl.exe), nothing is stopped unless every such PID is named in -AlsoStop:
       the person may be using the dev window, and their sessions are not the check's to end.
Nothing here deletes a folder. With -CodexAuth, start copies ~/.codex/auth.json into the scratch CODEX_HOME
       and stop deletes that copy. -CodexHome <short path> puts CODEX_HOME there instead of <Scratch>\cx; that
       folder is deleted with the scratch folder at the end of the work.
       start always creates <Scratch>\home\.gemini and starts the app with USERPROFILE=<Scratch>\home, so agy and
       the core's reads and writes of agy's settings.json use the scratch home; CARGO_HOME and RUSTUP_HOME are pinned
       to the real ones first. With -AgyAuth, it also copies ~/.gemini/oauth_creds.json and google_accounts.json into
       <Scratch>\home\.gemini (nothing else), and stop deletes those two copies.
       With -QwenFake, start also runs fake-openai.mjs (a scripted OpenAI-compatible model on 127.0.0.1:-QwenPort,
       logging every request to <Scratch>\qwen-fake.jsonl), points OPENAI_BASE_URL, OPENAI_API_KEY and OPENAI_MODEL
       at it for the app and its tabs, and writes <Scratch>\home\.qwen\settings.json selecting that provider, so
       Qwen Code starts with no account. stop ends the fake model by its recorded PID.

.EXAMPLE
pwsh -NoProfile -File live-check.ps1 start -Scratch C:\Users\me\AppData\Local\Temp\lc-5b-1
pwsh -NoProfile -File live-check.ps1 tree  -Scratch C:\Users\me\AppData\Local\Temp\lc-5b-1
pwsh -NoProfile -File live-check.ps1 stop  -Scratch C:\Users\me\AppData\Local\Temp\lc-5b-1 -AlsoStop 1234
#>
param(
    [Parameter(Mandatory, Position = 0)][ValidateSet('start', 'tree', 'stop')][string]$Mode,
    [Parameter(Mandatory)][string]$Scratch,
    [int[]]$AlsoStop = @(),
    # start only: a folder whose cc and cx subfolders are copied into the scratch CLAUDE_CONFIG_DIR and CODEX_HOME.
    [string]$Seed,
    # start only: copies ~/.codex/auth.json (and nothing else) into the scratch CODEX_HOME, so a real Codex can sign in.
    # stop deletes that one copy once the tree is gone.
    [switch]$CodexAuth,
    # start only: a new, short literal folder (at most 60 characters) to use as CODEX_HOME instead of <Scratch>\cx.
    # Codex 0.163 can't start its app-server under a longer one ("path must be shorter than SUN_LEN").
    [string]$CodexHome,
    # start only: copies ~/.gemini/oauth_creds.json and google_accounts.json (and nothing else) into the scratch
    # USERPROFILE's .gemini, so a real agy can sign in; stop deletes the two copies.
    [switch]$AgyAuth,
    # start only: runs the fake model for Qwen Code (slice 6l1 decision 8); stop ends it.
    [switch]$QwenFake,
    [int]$QwenPort = 47612,
    # The checkout to run; defaults to the one holding this script.
    [string]$Repo = (Join-Path $PSScriptRoot '..\..\..'),
    [int]$Port = 9333
)
$ErrorActionPreference = 'Stop'
$AgentNames = @('claude.exe', 'codex.exe', 'node_repl.exe', 'agy.exe')

if (-not [System.IO.Path]::IsPathRooted($Scratch)) { throw "Scratch must be an absolute path: $Scratch" }
# In PowerShell 7, GetFullPath also expands 8.3 short names (C:\Users\JANEDO~1) in the part that exists.
$Scratch = [System.IO.Path]::GetFullPath($Scratch).TrimEnd('\')
if ($CodexHome) {
    if (-not [System.IO.Path]::IsPathRooted($CodexHome)) { throw "CodexHome must be an absolute path: $CodexHome" }
    $CodexHome = [System.IO.Path]::GetFullPath($CodexHome).TrimEnd('\')
    if ($CodexHome.Length -gt 60) { throw "CodexHome is $($CodexHome.Length) characters; Codex needs at most 60: $CodexHome" }
}
foreach ($real in @("$HOME\.marshell", "$HOME\.claude", "$HOME\.codex", "$HOME\.gemini", "$HOME\.qwen")) {
    $r = [System.IO.Path]::GetFullPath($real).TrimEnd('\')
    foreach ($path in @($Scratch, $CodexHome) | Where-Object { $_ }) {
        if ($path -ieq $r -or $path.StartsWith("$r\", 'OrdinalIgnoreCase')) { throw "$path is inside a real config folder: $real" }
    }
}
if ((Split-Path $Scratch -Leaf) -ieq '.marshell') { throw 'Never name the scratch folder .marshell.' }
$PidsFile = Join-Path $Scratch 'pids.json'

function Get-Tree([int]$RootPid) {
    # PIDs 0 and 4 are the idle process and the kernel: every process descends from them.
    if ($RootPid -le 4) { throw "Refusing to walk the tree from PID $RootPid." }
    $all = Get-CimInstance Win32_Process -Property ProcessId, ParentProcessId, Name, CreationDate, CommandLine
    $byId = $all | Group-Object ProcessId -AsHashTable -AsString
    $byParent = $all | Group-Object ParentProcessId -AsHashTable -AsString
    $root = $all | Where-Object ProcessId -eq $RootPid
    if (-not $root) { return @() }
    $out = [System.Collections.Generic.List[object]]::new()
    $walk = {
        param($p, $depth)
        # Qwen Code runs as node.exe (cli-entry.js, then cli.js twice); only the topmost Qwen node counts as the agent.
        $parent = @($byId["$($p.ParentProcessId)"])[0]
        $qwen = $p.Name -ieq 'node.exe' -and $p.CommandLine -match 'qwen-code' -and
            -not ($parent -and $parent.Name -ieq 'node.exe' -and $parent.CommandLine -match 'qwen-code')
        $agent = ($AgentNames -contains $p.Name.ToLowerInvariant()) -or $qwen
        $out.Add([pscustomobject]@{ Depth = $depth; Name = $p.Name; PID = [int]$p.ProcessId; Parent = [int]$p.ParentProcessId; Agent = $agent })
        foreach ($c in @($byParent["$($p.ProcessId)"])) {
            # A child created before its parent is a reused parent PID, not a real child.
            if ($c -and $c.ProcessId -ne $p.ProcessId -and $c.CreationDate -ge $p.CreationDate) { & $walk $c ($depth + 1) }
        }
    }
    & $walk $root 0
    return $out
}

function Read-Record {
    if (-not (Test-Path -LiteralPath $PidsFile)) { throw "No $PidsFile; was start run with this Scratch?" }
    $rec = Get-Content -LiteralPath $PidsFile -Raw | ConvertFrom-Json
    $p = Get-CimInstance Win32_Process -Filter "ProcessId = $($rec.root)" -Property Name, CreationDate
    # Write-Host, not Write-Output: output would become part of the return value, so the caller would get a
    # non-null record and walk the tree from PID 0, the whole machine.
    if (-not $p) { Write-Host "Root PID $($rec.root) is no longer running."; return $null }
    # ConvertFrom-Json may already have turned `created` into a DateTime; compare as times either way.
    $recorded = ([datetime]$rec.created).ToUniversalTime()
    if ($p.Name -ne $rec.name -or [math]::Abs(($p.CreationDate.ToUniversalTime() - $recorded).TotalMilliseconds) -gt 1) {
        throw "PID $($rec.root) is now a different process ($($p.Name)); not touching it."
    }
    return $rec
}

# The copied sign-ins are the only files removed here: known literal names in the recorded scratch homes.
function Remove-AuthCopy($rec) {
    if ($rec.codexAuth -and $rec.codexHome) {
        $authCopy = Join-Path $rec.codexHome 'auth.json'
        if (Test-Path -LiteralPath $authCopy) {
            Remove-Item -LiteralPath $authCopy -Force
            Write-Output "Deleted the copied Codex sign-in: $authCopy"
        }
    }
    if ($rec.agyAuth -and $rec.agyHome) {
        foreach ($name in @('oauth_creds.json', 'google_accounts.json')) {
            $copy = Join-Path $rec.agyHome ".gemini\$name"
            if (Test-Path -LiteralPath $copy) {
                Remove-Item -LiteralPath $copy -Force
                Write-Output "Deleted the copied agy sign-in: $copy"
            }
        }
    }
}

# The fake model runs outside the app's tree: it is stopped by its recorded PID, after checking it is still that process.
function Stop-QwenFake($rec) {
    if (-not $rec.qwenFake.pid) { return }
    $p = Get-CimInstance Win32_Process -Filter "ProcessId = $($rec.qwenFake.pid)" -Property Name, CreationDate, CommandLine
    if (-not $p) { Write-Output "The fake model (pid $($rec.qwenFake.pid)) is already gone."; return }
    if ($p.Name -ne 'node.exe' -or $p.CommandLine -notlike '*fake-openai.mjs*') {
        Write-Output "PID $($rec.qwenFake.pid) is now $($p.Name), not the fake model; not touching it."; return
    }
    Stop-Process -Id $rec.qwenFake.pid -Force
    Write-Output "Stopped the fake model, pid $($rec.qwenFake.pid)."
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
        if ($CodexHome) {
            if ((Test-Path -LiteralPath $CodexHome) -and (Get-ChildItem -LiteralPath $CodexHome -Force | Select-Object -First 1)) {
                throw "CodexHome is not empty: $CodexHome. Use a new literal path."
            }
            $cx = if (Test-Path -LiteralPath $CodexHome) { Get-Item -LiteralPath $CodexHome } else { New-Item -ItemType Directory -Path $CodexHome }
        } else {
            $cx = New-Item -ItemType Directory -Path (Join-Path $Scratch 'cx')
        }
        if ($Seed) {
            foreach ($pair in @(@('cc', $cc), @('cx', $cx))) {
                $from = Join-Path $Seed $pair[0]
                if (Test-Path -LiteralPath $from) { Copy-Item -Path (Join-Path $from '*') -Destination $pair[1].FullName -Recurse }
            }
        }
        if ($CodexAuth) {
            $auth = Join-Path $HOME '.codex/auth.json'
            if (-not (Test-Path -LiteralPath $auth)) { throw "No Codex sign-in to copy: $auth" }
            Copy-Item -LiteralPath $auth -Destination (Join-Path $cx.FullName 'auth.json')
        }
        # Always a scratch home: agy has no home variable of its own, and the app writes agy's settings.json.
        $agyHome = (New-Item -ItemType Directory -Path (Join-Path $Scratch 'home')).FullName
        $gemini = (New-Item -ItemType Directory -Path (Join-Path $agyHome '.gemini')).FullName
        if ($AgyAuth) {
            foreach ($name in @('oauth_creds.json', 'google_accounts.json')) {
                $from = Join-Path $HOME ".gemini\$name"
                if (-not (Test-Path -LiteralPath $from)) { throw "No agy sign-in to copy: $from" }
                Copy-Item -LiteralPath $from -Destination (Join-Path $gemini $name)
            }
        }
        $fakeRec = $null
        if ($QwenFake) {
            $qwenDir = (New-Item -ItemType Directory -Path (Join-Path $agyHome '.qwen')).FullName
            Set-Content -LiteralPath (Join-Path $qwenDir 'settings.json') -Encoding utf8NoBOM `
                -Value '{ "security": { "auth": { "selectedType": "openai" } }, "model": { "name": "marshell-fake" }, "general": { "enableAutoUpdate": false } }'
            # Without enableAutoUpdate false, a Qwen session that found a newer version updates the real install
            # when it exits. The variable also skips the check, in case a check rewrites the settings file.
            $env:QWEN_CODE_SKIP_UPDATE_CHECK_ONCE = 'true'
            $fake = Start-Process -FilePath 'node' -ArgumentList "`"$(Join-Path $PSScriptRoot 'fake-openai.mjs')`"", $QwenPort, "`"$(Join-Path $Scratch 'qwen-fake.jsonl')`"" `
                -NoNewWindow -PassThru -RedirectStandardOutput (Join-Path $Scratch 'qwen-fake.out.log') -RedirectStandardError (Join-Path $Scratch 'qwen-fake.err.log')
            $fakeRec = [ordered]@{ pid = $fake.Id; port = $QwenPort }
            $env:OPENAI_API_KEY = 'marshell-fake'
            $env:OPENAI_BASE_URL = "http://127.0.0.1:$QwenPort/v1"
            $env:OPENAI_MODEL = 'marshell-fake'
        }
        $settings = Join-Path (New-Item -ItemType Directory -Path (Join-Path $Scratch 'proj\.claude')).FullName 'settings.local.json'
        Set-Content -LiteralPath $settings -Value '{}' -Encoding utf8NoBOM
        $repo = (Resolve-Path -LiteralPath $Repo).Path
        if (-not (Test-Path -LiteralPath (Join-Path $repo 'src-tauri\tauri.conf.json'))) { throw "Not a Marshell checkout: $repo" }
        $env:MARSHELL_HOME = $mh.FullName
        $env:CLAUDE_CONFIG_DIR = $cc.FullName
        $env:CODEX_HOME = $cx.FullName
        $env:MARSHELL_CLAUDE_SETTINGS = $settings
        # Cargo and rustup find their homes through USERPROFILE too; keep the real ones for the build.
        if (-not $env:CARGO_HOME) { $env:CARGO_HOME = Join-Path $HOME '.cargo' }
        if (-not $env:RUSTUP_HOME) { $env:RUSTUP_HOME = Join-Path $HOME '.rustup' }
        $env:USERPROFILE = $agyHome
        $env:WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS = "--remote-debugging-port=$Port"
        $proc = Start-Process -FilePath 'cmd.exe' -ArgumentList '/d', '/c', 'pnpm tauri dev' -WorkingDirectory $repo `
            -NoNewWindow -PassThru -RedirectStandardOutput (Join-Path $Scratch 'dev.out.log') `
            -RedirectStandardError (Join-Path $Scratch 'dev.err.log')
        $created = (Get-CimInstance Win32_Process -Filter "ProcessId = $($proc.Id)" -Property CreationDate).CreationDate
        [ordered]@{
            root = $proc.Id; name = 'cmd.exe'; created = $created.ToUniversalTime().ToString('o')
            repo = $repo; marshellHome = $mh.FullName; claudeConfigDir = $cc.FullName; codexHome = $cx.FullName; codexAuth = [bool]$CodexAuth; agyAuth = [bool]$AgyAuth; agyHome = $agyHome; qwenFake = $fakeRec; claudeSettings = $settings; cdp = "http://127.0.0.1:$Port"
        } | ConvertTo-Json | Set-Content -LiteralPath $PidsFile -Encoding utf8NoBOM
        Write-Output "Started pnpm tauri dev in $repo, root PID $($proc.Id)."
        Write-Output "MARSHELL_HOME=$($mh.FullName)"
        Write-Output "CLAUDE_CONFIG_DIR=$($cc.FullName)"
        Write-Output "CODEX_HOME=$($cx.FullName)"
        Write-Output "MARSHELL_CLAUDE_SETTINGS=$settings"
        Write-Output "USERPROFILE=$agyHome (agy's and Qwen's scratch home)"
        if ($fakeRec) { Write-Output "Fake model for Qwen: pid $($fakeRec.pid), http://127.0.0.1:$QwenPort/v1, log $Scratch\qwen-fake.jsonl" }
        Write-Output "Logs: $Scratch\dev.out.log, $Scratch\dev.err.log. CDP: http://127.0.0.1:$Port/json/version"
    }
    'tree' {
        $rec = Read-Record
        if ($rec) { Show-Tree (Get-Tree $rec.root) }
    }
    'stop' {
        $rec = Read-Record
        if (-not $rec) {
            # The tree is gone already (stopped some other way), but the sign-in copy may not be.
            $old = Get-Content -LiteralPath $PidsFile -Raw | ConvertFrom-Json
            Remove-AuthCopy $old
            Stop-QwenFake $old
            return
        }
        $tree = @(Get-Tree $rec.root)
        Show-Tree $tree
        $agents = @($tree | Where-Object Agent)
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
        Remove-AuthCopy $rec
        Stop-QwenFake $rec
        Write-Output "All $($tree.Count) recorded processes are gone. $Scratch was left in place."
    }
}
