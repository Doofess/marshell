#Requires -Version 7
<#
.SYNOPSIS
  Release smoke test: installs the alpha, starts it, updates over it, uninstalls it, and checks what each step left
  behind. It runs on the release workflow's runner. It installs and uninstalls Marshell for the current user, and each
  uninstall takes Marshell's hooks out of the CLI files the way a real one does: also out of ~/.gemini, ~/.agents and
  every WSL distro, which no variable moves. So it refuses to run anywhere but GitHub Actions unless
  -ThrowawayMachine says the machine holds nothing to lose, and it refuses where Marshell is installed or running.

.DESCRIPTION
  Exit 0 when every check passed, 1 otherwise. Every check prints exactly one PASS, FAIL or SKIP line. A failed check
  does not stop the later ones unless they cannot run without it. The scratch folder is left in place.

  Everything the script starts runs with MARSHELL_HOME, MARSHELL_CLAUDE_SETTINGS, CLAUDE_CONFIG_DIR, CODEX_HOME,
  COPILOT_HOME and QWEN_HOME pointing into the scratch folder. The script never reads run/endpoint.json or a token: it asks the app
  only through `marshell list --json`.

.PARAMETER Version
  The version the installer carries, as DisplayVersion and `marshell --version` show it.
.PARAMETER Installer
  The setup file (Marshell_<version>_x64-setup.exe). Runs with /S.
.PARAMETER WingetManifest
  A folder with a winget manifest; installs with `winget install --manifest`.
.PARAMETER WingetId
  A package id (Doofess.Marshell); installs with `winget install --id <id> --version <v>`.
.PARAMETER Fixture
  smoke-fixture.exe (cargo build --release -p marshell-cli --example smoke-fixture). Without it the hook checks SKIP.
.PARAMETER ThrowawayMachine
  Allows a run outside GitHub Actions. Only for a VM that is thrown away afterwards.
.PARAMETER Scratch
  A folder that does not exist yet, under the user profile (the config guard allows only that). Default: a new
  folder under $env:TEMP.
#>
[CmdletBinding()]
param(
    [Parameter(Mandatory)][string]$Version,
    [string]$Installer,
    [string]$WingetManifest,
    [string]$WingetId,
    [string]$Fixture,
    [string]$Scratch,
    [switch]$ThrowawayMachine
)

$ErrorActionPreference = 'Stop'
$global:LASTEXITCODE = 0

$UninstKey = 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Uninstall\Marshell'
$HklmKeys = @(
    'HKLM:\Software\Microsoft\Windows\CurrentVersion\Uninstall\Marshell',
    'HKLM:\Software\WOW6432Node\Microsoft\Windows\CurrentVersion\Uninstall\Marshell'
)
$AppName = 'marshell-app'
$Programs = @('marshell-app.exe', 'marshell.exe', 'marshell-hook.exe', 'marshell-host.exe', 'conpty.dll', 'OpenConsole.exe', 'uninstall.exe', 'LICENSE', 'THIRD_PARTY_NOTICES.md')
$KnownText = "{`n  `"theme`": `"dark`",`n  `"statusLine`": {`"type`": `"command`", `"command`": `"my-line`"}`n}`n"
$KnownBytes = (New-Object System.Text.UTF8Encoding($false)).GetBytes($KnownText)
$BrokenBytes = (New-Object System.Text.UTF8Encoding($false)).GetBytes("{ `"hooks`": [oops`n")

$script:Pass = 0
$script:Fail = 0
$script:Skip = 0
$script:Started = New-Object System.Collections.Generic.List[int]
$script:Inst = $null
$script:App = $null

# ---------------------------------------------------------------- reporting

function Report-Pass([string]$Name) { $script:Pass++; Write-Host "PASS $Name" }
function Report-Fail([string]$Name, [string]$Why) {
    $script:Fail++
    if ($Why) { Write-Host "FAIL ${Name}: $Why" } else { Write-Host "FAIL $Name" }
}
function Report-Skip([string]$Name, [string]$Why) { $script:Skip++; Write-Host "SKIP ${Name}: $Why" }

# Runs $Test. It passes when the last value it outputs is $true; a string it outputs is the reason it failed.
function Check([string]$Name, [scriptblock]$Test) {
    try {
        $checkVals = @(& $Test)
        $checkResult = if ($checkVals.Count -gt 0) { $checkVals[-1] } else { $null }
    } catch {
        $checkResult = "error: $($_.Exception.Message)"
    }
    if ($checkResult -is [bool] -and $checkResult) { Report-Pass $Name }
    elseif ($checkResult -is [string]) { Report-Fail $Name $checkResult }
    else { Report-Fail $Name '' }
}

function Short([string]$Text) {
    $lines = @(($Text -split "`r?`n") | Where-Object { $_.Trim() } | Select-Object -Last 6)
    return ($lines -join ' | ')
}

# ---------------------------------------------------------------- helpers

function Wait-Until([scriptblock]$Cond, [int]$Seconds, [int]$StepMs = 1000) {
    $sw = [System.Diagnostics.Stopwatch]::StartNew()
    while ($true) {
        if (& $Cond) { return $true }
        if ($sw.Elapsed.TotalSeconds -ge $Seconds) { return $false }
        Start-Sleep -Milliseconds $StepMs
    }
}

# Starts a program and waits for it up to $TimeoutSec. Gives its exit code, or $null when it was still running (it is
# then stopped, by the PID this started).
function Invoke-Setup([string]$File, [string]$ArgString, [int]$TimeoutSec) {
    $p = if ($ArgString) {
        Start-Process -FilePath $File -ArgumentList $ArgString -PassThru -WindowStyle Hidden
    } else {
        Start-Process -FilePath $File -PassThru -WindowStyle Hidden
    }
    $null = $p.Handle
    if (-not $p.WaitForExit($TimeoutSec * 1000)) {
        Stop-Process -Id $p.Id -Force -ErrorAction SilentlyContinue
        return $null
    }
    return $p.ExitCode
}

function Invoke-Native([string]$File, [string[]]$Arguments) {
    $out = & $File @Arguments 2>&1 | Out-String
    return [pscustomobject]@{ Code = $LASTEXITCODE; Output = $out.Trim() }
}

function Get-Uninst {
    if (Test-Path -LiteralPath $UninstKey) { return Get-ItemProperty -LiteralPath $UninstKey }
    return $null
}

# The folder the uninstall key names. The value may be quoted and may end in a backslash.
function Get-InstallDir {
    $k = Get-Uninst
    if (-not $k) { return $null }
    $v = $k.InstallLocation
    if (-not $v) { return $null }
    return ([string]$v).Trim().Trim('"').TrimEnd('\')
}

function Test-AppAlive([int]$ProcId) {
    $q = Get-Process -Id $ProcId -ErrorAction SilentlyContinue
    return [bool]($q -and $q.ProcessName -eq $AppName)
}

function Wait-AppGone([int]$ProcId, [int]$Seconds) {
    return (Wait-Until { -not (Test-AppAlive $ProcId) } $Seconds)
}

function Test-Contains([string]$Path, [string]$Text) {
    if (-not (Test-Path -LiteralPath $Path)) { return $false }
    return [System.IO.File]::ReadAllText($Path).Contains($Text)
}

function Get-BytesHash([byte[]]$Bytes) {
    return [System.Convert]::ToHexString([System.Security.Cryptography.SHA256]::HashData($Bytes))
}

function Test-FileBytes([string]$Path, [byte[]]$Want) {
    if (-not (Test-Path -LiteralPath $Path)) { return $false }
    return ((Get-FileHash -LiteralPath $Path -Algorithm SHA256).Hash -eq (Get-BytesHash $Want))
}

function Install-Marshell {
    switch ($script:Mode) {
        'installer' {
            $code = Invoke-Setup $Installer '/S' 300
            return [pscustomobject]@{ Code = $code; Output = '' }
        }
        'manifest' {
            return Invoke-Native 'winget' @('install', '--manifest', $WingetManifest, '--silent', '--accept-package-agreements', '--accept-source-agreements', '--disable-interactivity')
        }
        default {
            return Invoke-Native 'winget' @('install', '--id', $WingetId, '--exact', '--version', $Version, '--source', 'winget', '--silent', '--accept-package-agreements', '--accept-source-agreements', '--disable-interactivity')
        }
    }
}

# The uninstaller's copy of itself: NSIS runs it from a folder named ~nsu<x>.tmp under the temp folder.
function Get-UninstallerCopies {
    return @(Get-Process -ErrorAction SilentlyContinue | Where-Object { $_.Path -and $_.Path -like '*\~nsu*.tmp\*' })
}

# Starts the uninstall the way a person would. The uninstaller hands over to a copy of itself in the temp folder and
# returns early, so this waits up to 120 s for the uninstall key to go, and then up to 180 s for that copy to end:
# it takes the hooks out after the key is gone. Gives the first command's result.
function Uninstall-Marshell {
    switch ($script:Mode) {
        'installer' {
            # Exactly what the key says, as winget runs it.
            $k = Get-Uninst
            $q = if ($k) { [string]$k.QuietUninstallString } else { '' }
            if ($q -notmatch '^\s*"([^"]+)"\s*(.*)$') { throw "the uninstall key has no usable QuietUninstallString: '$q'" }
            $code = Invoke-Setup $Matches[1] $Matches[2] 120
            $res = [pscustomobject]@{ Code = $code; Output = '' }
        }
        'manifest' {
            # An install from a manifest folder isn't tied to a package id until the package is in the community
            # repository, so the release notes give this form. What `winget list` says about the id is printed.
            $listed = Invoke-Native 'winget' @('list', '--id', 'Doofess.Marshell', '--exact', '--disable-interactivity')
            Write-Host "NOTE winget list --id Doofess.Marshell --exact exited $($listed.Code) after an install from a manifest folder"
            $res = Invoke-Native 'winget' @('uninstall', '--manifest', $WingetManifest, '--silent', '--disable-interactivity')
        }
        default {
            $res = Invoke-Native 'winget' @('uninstall', '--id', $WingetId, '--exact', '--silent', '--disable-interactivity')
        }
    }
    $null = Wait-Until { -not (Test-Path -LiteralPath $UninstKey) } 120 2000
    $ended = Wait-Until { (Get-UninstallerCopies).Count -eq 0 } 180 1000
    if (-not $ended) { Write-Host 'NOTE the uninstaller was still running 180 s after the uninstall key went' }
    return $res
}

# The app the installer may have started on its own is ours too (the refusal at the start means none ran before).
function Start-App([string]$Dir) {
    $existing = @(Get-Process -Name $AppName -ErrorAction SilentlyContinue)
    if ($existing.Count -gt 0) {
        foreach ($e in $existing) { if (-not $script:Started.Contains($e.Id)) { $script:Started.Add($e.Id) } }
        return $existing[0]
    }
    $p = Start-Process -FilePath (Join-Path $Dir 'marshell-app.exe') -WorkingDirectory $Dir -PassThru
    $script:Started.Add($p.Id)
    return $p
}

# True when the installed marshell.exe gets an answer from the app within 90 s; else the reason.
function Wait-AppReady($Proc, [string]$Dir) {
    $cli = Join-Path $Dir 'marshell.exe'
    $script:AppWhy = ''
    $ok = Wait-Until {
        if (-not (Test-AppAlive $Proc.Id)) { $script:AppWhy = "marshell-app (PID $($Proc.Id)) is not running"; return $true }
        & $cli list --json *> $null
        $LASTEXITCODE -eq 0
    } 90 2000
    if ($script:AppWhy) { return $script:AppWhy }
    if ($ok) { return $true }
    return 'marshell list --json did not exit 0 within 90 s'
}

function Stop-Started {
    foreach ($id in $script:Started) {
        if (Test-AppAlive $id) {
            Stop-Process -Id $id -Force -ErrorAction SilentlyContinue
            Write-Host "NOTE stopped marshell-app PID $id, which this script started"
        }
    }
}

# ---------------------------------------------------------------- checks run before anything

if ($env:GITHUB_ACTIONS -ne 'true' -and -not $ThrowawayMachine) {
    Write-Host 'FAIL refused to run: this is not a GitHub Actions runner. The uninstalls take Marshell hooks out of the real ~/.gemini, ~/.agents and WSL distros. Nothing was changed.'
    exit 1
}
if (-not $Version.Trim()) { Write-Host 'FAIL arguments: -Version is empty'; exit 1 }
$modes = @()
if ($Installer) { $modes += 'installer' }
if ($WingetManifest) { $modes += 'manifest' }
if ($WingetId) { $modes += 'winget' }
if ($modes.Count -ne 1) {
    Write-Host 'FAIL arguments: give exactly one of -Installer, -WingetManifest or -WingetId'
    exit 1
}
$script:Mode = $modes[0]
if ($Installer -and -not (Test-Path -LiteralPath $Installer -PathType Leaf)) { Write-Host "FAIL arguments: no setup file at $Installer"; exit 1 }
if ($WingetManifest -and -not (Test-Path -LiteralPath $WingetManifest -PathType Container)) { Write-Host "FAIL arguments: no manifest folder at $WingetManifest"; exit 1 }
if ($Fixture -and -not (Test-Path -LiteralPath $Fixture -PathType Leaf)) { Write-Host "FAIL arguments: no fixture at $Fixture"; exit 1 }
if ($Installer) { $Installer = (Resolve-Path -LiteralPath $Installer).ProviderPath }
if ($WingetManifest) { $WingetManifest = (Resolve-Path -LiteralPath $WingetManifest).ProviderPath }
if ($Fixture) { $Fixture = (Resolve-Path -LiteralPath $Fixture).ProviderPath }

if (Test-Path -LiteralPath $UninstKey) {
    Write-Host "FAIL refused to run: Marshell is installed already ($UninstKey exists). Nothing was changed."
    exit 1
}
$already = @(Get-Process -Name $AppName -ErrorAction SilentlyContinue)
if ($already.Count -gt 0) {
    Write-Host "FAIL refused to run: marshell-app is running (PID $($already[0].Id)). Nothing was changed."
    exit 1
}

if (-not $Scratch) { $Scratch = Join-Path $env:TEMP ('marshell-smoke-' + [guid]::NewGuid().ToString('N').Substring(0, 8)) }
$Scratch = [System.IO.Path]::GetFullPath($Scratch)
if (Test-Path -LiteralPath $Scratch) { Write-Host "FAIL arguments: the scratch folder $Scratch exists already"; exit 1 }
if (-not $Scratch.StartsWith($env:USERPROFILE.TrimEnd('\') + '\', [System.StringComparison]::OrdinalIgnoreCase)) {
    Write-Host "FAIL arguments: the scratch folder $Scratch is not under $env:USERPROFILE, where the config guard allows files"
    exit 1
}

$MHome = Join-Path $Scratch 'marshell-home'
$ClaudeDir = Join-Path $Scratch 'claude'
$ClaudeFile = Join-Path $ClaudeDir 'settings.json'
$CodexDir = Join-Path $Scratch 'codex'
$CodexFile = Join-Path $CodexDir 'hooks.json'
$CopilotDir = Join-Path $Scratch 'copilot'
$QwenDir = Join-Path $Scratch 'qwen'
foreach ($d in @($Scratch, $MHome, $ClaudeDir, $CodexDir, $CopilotDir, $QwenDir)) { $null = New-Item -ItemType Directory -Path $d }

$env:MARSHELL_HOME = $MHome
$env:MARSHELL_CLAUDE_SETTINGS = $ClaudeFile
$env:CLAUDE_CONFIG_DIR = $ClaudeDir
$env:CODEX_HOME = $CodexDir
$env:COPILOT_HOME = $CopilotDir
$env:QWEN_HOME = $QwenDir
Write-Host "NOTE scratch folder $Scratch, mode $($script:Mode), version $Version"

function Write-KnownSettings { [System.IO.File]::WriteAllBytes($ClaudeFile, $KnownBytes) }

# ---------------------------------------------------------------- one install, checked

function Test-InstallState {
    Check 'install: the uninstall key is under HKCU' { [bool](Get-Uninst) }
    Check 'install: no uninstall key under HKLM' {
        foreach ($k in $HklmKeys) { if (Test-Path -LiteralPath $k) { return "found $k" } }
        $true
    }
    Check 'install: DisplayName is Marshell' { $v = (Get-Uninst).DisplayName; if ($v -eq 'Marshell') { $true } else { "it is '$v'" } }
    Check 'install: Publisher is Doofess' { $v = (Get-Uninst).Publisher; if ($v -eq 'Doofess') { $true } else { "it is '$v'" } }
    Check 'install: DisplayVersion is the version' { $v = (Get-Uninst).DisplayVersion; if ($v -eq $Version) { $true } else { "it is '$v', wanted '$Version'" } }
    $script:Inst = Get-InstallDir
    Check 'install: QuietUninstallString is the uninstaller with /S' {
        $v = [string](Get-Uninst).QuietUninstallString
        $want = '"' + $script:Inst + '\uninstall.exe" /S'
        if ($v -eq $want) { $true } else { "it is '$v', wanted '$want'" }
    }
    Check 'install: the folder is under LOCALAPPDATA' {
        if (-not $script:Inst) { return 'the key has no InstallLocation' }
        if ($script:Inst.StartsWith($env:LOCALAPPDATA.TrimEnd('\') + '\', [System.StringComparison]::OrdinalIgnoreCase)) { $true } else { "it is $($script:Inst)" }
    }
    Check 'install: the folder holds every program' {
        if (-not $script:Inst) { return 'no install folder' }
        $missing = @($Programs | Where-Object { -not (Test-Path -LiteralPath (Join-Path $script:Inst $_) -PathType Leaf) })
        if ($missing.Count -eq 0) { $true } else { 'missing ' + ($missing -join ', ') }
    }
    Check 'install: marshell.exe --version prints the version' {
        $r = Invoke-Native (Join-Path $script:Inst 'marshell.exe') @('--version')
        if ($r.Code -eq 0 -and $r.Output -eq "marshell $Version") { $true } else { "exit $($r.Code), printed '$($r.Output)'" }
    }
}

# Install, check the install, start the app and wait for it. Gives $true when the app answers.
function Start-Cycle([string]$Label) {
    $r = Install-Marshell
    Check "$Label install: the installer exited 0" {
        if ($r.Code -eq 0) { $true } else { "exit code '$($r.Code)' $(Short $r.Output)" }
    }
    Test-InstallState
    if (-not $script:Inst -or -not (Test-Path -LiteralPath (Join-Path $script:Inst 'marshell-app.exe'))) {
        Report-Fail "$Label start: the app" 'there is no installed app to start'
        return $false
    }
    $script:App = Start-App $script:Inst
    $ready = Wait-AppReady $script:App $script:Inst
    Check "$Label start: marshell list --json answers within 90 s" { $ready }
    Check "$Label start: the home bin holds marshell-hook.exe and marshell.exe" {
        $bin = Join-Path $MHome 'bin'
        $missing = @('marshell-hook.exe', 'marshell.exe' | Where-Object { -not (Test-Path -LiteralPath (Join-Path $bin $_) -PathType Leaf) })
        if ($missing.Count -eq 0) { $true } else { 'missing ' + ($missing -join ', ') }
    }
    return ($ready -is [bool] -and $ready)
}

# Checks after an uninstall that began with the app open or closed. $InstDir is the folder it was installed in.
function Test-Uninstalled([string]$Label, [string]$InstDir, $Res, [int]$AppId) {
    Check "$Label uninstall: the command ran and exited 0" {
        if ($Res.Code -eq 0) { $true } else { "exit code '$($Res.Code)' $(Short $Res.Output)" }
    }
    Check "$Label uninstall: the uninstall key is gone within 120 s" {
        if (-not (Test-Path -LiteralPath $UninstKey)) { $true } else { 'the key is still there' }
    }
    if ($AppId -gt 0) {
        Check "$Label uninstall: the app has ended" {
            if (Wait-AppGone $AppId 30) { $true } else { "PID $AppId is still running" }
        }
    }
    Check "$Label uninstall: the install folder holds no file" {
        if (-not $InstDir) { return 'the install folder is not known' }
        $left = $null
        $gone = Wait-Until {
            $script:left = @(Get-ChildItem -LiteralPath $InstDir -Recurse -File -Force -ErrorAction SilentlyContinue)
            $script:left.Count -eq 0
        } 30
        if ($gone) { $true } else { 'left ' + (($script:left | Select-Object -First 5 | ForEach-Object { $_.Name }) -join ', ') }
    }
}

# ---------------------------------------------------------------- cycle A

function Invoke-CycleA {
    $up = Start-Cycle 'A1'
    if (-not $script:Inst) {
        Report-Skip 'A: the rest of cycle A' 'the install did not leave an install folder'
        return
    }
    if (-not $up) {
        Report-Skip 'A: the rest of cycle A' 'the app did not start or answer'
        if ($script:App -and (Test-AppAlive $script:App.Id)) { Stop-Process -Id $script:App.Id -Force -ErrorAction SilentlyContinue }
        return
    }

    # 4: the CLI files, the fixture.
    Write-KnownSettings
    if ($Fixture) {
        $fx = Invoke-Native $Fixture @($MHome)
        Check 'A4 fixture: smoke-fixture exited 0' { if ($fx.Code -eq 0) { $true } else { "exit $($fx.Code): $(Short $fx.Output)" } }
        Check "A4 fixture: Claude Code's settings.json names marshell-hook" { Test-Contains $ClaudeFile 'marshell-hook' }
        Check "A4 fixture: Codex's hooks.json names marshell-hook" { Test-Contains $CodexFile 'marshell-hook' }
    } else {
        Report-Skip 'A4 fixture: smoke-fixture and the hook checks' 'no -Fixture given'
    }

    $oldId = $script:App.Id
    if ($script:Mode -eq 'installer') {
        # 5: install again over the running app, the way winget upgrade does.
        $r = Install-Marshell
        Check 'A5 reinstall over the running app: the installer exited 0' { if ($r.Code -eq 0) { $true } else { "exit code '$($r.Code)'" } }
        Check 'A5 reinstall over the running app: the app has ended' { if (Wait-AppGone $oldId 60) { $true } else { "PID $oldId is still running" } }
        if ($Fixture) {
            Check "A5 reinstall over the running app: Claude Code's file still names marshell-hook" { Test-Contains $ClaudeFile 'marshell-hook' }
        } else {
            Report-Skip "A5 reinstall over the running app: Claude Code's file" 'no -Fixture given'
        }

        # 5b: update the way a newer installer run by hand does: the old uninstaller in place, then the setup file.
        $dir = Get-InstallDir
        if (-not $dir) {
            Report-Fail 'A5b in-place uninstall: the install folder' 'the uninstall key has no InstallLocation'
        } else {
            $code = Invoke-Setup (Join-Path $dir 'uninstall.exe') "/S _?=$dir" 300
            Check 'A5b in-place uninstall: uninstall.exe exited 0' { if ($code -eq 0) { $true } else { "exit code '$code'" } }
            if ($Fixture) {
                Check "A5b in-place uninstall: Claude Code's file still names marshell-hook" { Test-Contains $ClaudeFile 'marshell-hook' }
                Check 'A5b in-place uninstall: the home bin still holds marshell-hook.exe' { Test-Path -LiteralPath (Join-Path $MHome 'bin\marshell-hook.exe') -PathType Leaf }
            } else {
                Report-Skip 'A5b in-place uninstall: the hook checks' 'no -Fixture given'
            }
            $r = Install-Marshell
            Check 'A5b setup again: the installer exited 0' { if ($r.Code -eq 0) { $true } else { "exit code '$($r.Code)'" } }
            Check 'A5b setup again: the uninstall key is back' { [bool](Get-Uninst) }
            $script:Inst = Get-InstallDir
        }
    } else {
        foreach ($n in 'A5 reinstall over the running app', 'A5b in-place uninstall and setup') {
            Report-Skip $n 'only the -Installer mode runs it'
        }
    }

    # 6: the app again.
    if (-not $script:Inst) {
        Report-Skip 'A6 start again and the rest' 'no install folder after the update'
        return
    }
    if ($script:App -and (Test-AppAlive $script:App.Id)) {
        $app = $script:App
    } else {
        $app = Start-App $script:Inst
        $script:App = $app
    }
    $ready = Wait-AppReady $app $script:Inst
    Check 'A6 start again: marshell list --json answers within 90 s' { $ready }

    # 7 and 8: uninstall with the app open. The hooks must still be there, or step 8 would prove nothing.
    if ($Fixture) {
        Check "A6 before the uninstall: Claude Code's and Codex's files still name marshell-hook" {
            (Test-Contains $ClaudeFile 'marshell-hook') -and (Test-Contains $CodexFile 'marshell-hook')
        }
    }
    $instDir = $script:Inst
    $appId = $app.Id
    $res = Uninstall-Marshell
    Test-Uninstalled 'A7' $instDir $res $appId
    if ($Fixture) {
        Check "A8 after uninstall: Claude Code's settings.json is the known one, byte for byte" {
            if (Test-FileBytes $ClaudeFile $KnownBytes) { $true } else { 'the bytes differ' }
        }
        Check "A8 after uninstall: Codex's hooks.json is gone or doesn't name marshell-hook" {
            if (-not (Test-Path -LiteralPath $CodexFile)) { return $true }
            if (-not (Test-Contains $CodexFile 'marshell-hook')) { return $true }
            'it still names marshell-hook'
        }
    } else {
        Report-Skip 'A8 after uninstall: the hook files' 'no -Fixture given'
    }
    Check 'A8 after uninstall: the home bin is gone' {
        $bin = Join-Path $MHome 'bin'
        if (Wait-Until { -not (Test-Path -LiteralPath $bin) } 30) { $true } else { "$bin is still there" }
    }
    Check 'A8 after uninstall: the home snapshots folder is still there' { Test-Path -LiteralPath (Join-Path $MHome 'snapshots') -PathType Container }
    Check 'A8 after uninstall: HKCU Classes marshell does not point into the install folder' {
        $k = 'HKCU:\Software\Classes\marshell\shell\open\command'
        if (-not (Test-Path -LiteralPath $k)) { return $true }
        $v = [string](Get-Item -LiteralPath $k).GetValue('')
        if ($instDir -and $v.IndexOf($instDir, [System.StringComparison]::OrdinalIgnoreCase) -ge 0) { "it is $v" } else { $true }
    }
    Check 'A8 after uninstall: the Start menu has no Marshell.lnk' {
        $menu = Join-Path $env:APPDATA 'Microsoft\Windows\Start Menu\Programs'
        $found = @(Get-ChildItem -LiteralPath $menu -Recurse -Filter 'Marshell.lnk' -ErrorAction SilentlyContinue)
        if ($found.Count -eq 0) { $true } else { "found $($found[0].FullName)" }
    }
}

# ---------------------------------------------------------------- cycle B

function Invoke-CycleB {
    if (-not $Fixture) {
        Report-Skip 'B: a file that cannot be changed' 'no -Fixture given'
        return
    }
    if (Test-Path -LiteralPath $UninstKey) {
        Report-Skip 'B: a file that cannot be changed' 'cycle A left Marshell installed'
        return
    }
    $up = Start-Cycle 'B1'
    if (-not $script:Inst) {
        Report-Skip 'B: the rest of cycle B' 'the install did not leave an install folder'
        return
    }
    if (-not $up) {
        Report-Skip 'B: the rest of cycle B' 'the app did not start or answer'
        if ($script:App -and (Test-AppAlive $script:App.Id)) { Stop-Process -Id $script:App.Id -Force -ErrorAction SilentlyContinue }
        return
    }
    $appId = $script:App.Id
    Stop-Process -Id $appId -Force -ErrorAction SilentlyContinue
    Check 'B2 stop the app by its PID: it has ended' { if (Wait-AppGone $appId 30) { $true } else { "PID $appId is still running" } }

    Write-KnownSettings
    if (Test-Path -LiteralPath $CodexFile) { Remove-Item -LiteralPath $CodexFile -Force }
    $fx = Invoke-Native $Fixture @($MHome)
    Check 'B3 fixture: smoke-fixture exited 0' { if ($fx.Code -eq 0) { $true } else { "exit $($fx.Code): $(Short $fx.Output)" } }
    [System.IO.File]::WriteAllBytes($CodexFile, $BrokenBytes)

    $instDir = $script:Inst
    $res = Uninstall-Marshell
    Test-Uninstalled 'B4' $instDir $res 0
    Check "B5 after uninstall: Claude Code's settings.json is the known one, byte for byte" {
        if (Test-FileBytes $ClaudeFile $KnownBytes) { $true } else { 'the bytes differ' }
    }
    Check "B5 after uninstall: Codex's hooks.json is the broken file, byte for byte" {
        if (Test-FileBytes $CodexFile $BrokenBytes) { $true } else { 'the bytes differ' }
    }
    Check 'B5 after uninstall: the home bin still holds marshell-hook.exe' {
        Test-Path -LiteralPath (Join-Path $MHome 'bin\marshell-hook.exe') -PathType Leaf
    }
}

# ---------------------------------------------------------------- run

try {
    Invoke-CycleA
    Invoke-CycleB
} catch {
    Report-Fail 'smoke: the script itself' $_.Exception.Message
} finally {
    Stop-Started
}

if (Test-Path -LiteralPath $UninstKey) {
    Write-Host "NOTE Marshell is still installed ($UninstKey exists)."
}
Write-Host "RESULT $($script:Pass) passed, $($script:Fail) failed, $($script:Skip) skipped"
Write-Host "Scratch folder: $Scratch"
if ($script:Fail -eq 0) { exit 0 } else { exit 1 }
