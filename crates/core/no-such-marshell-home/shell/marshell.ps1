# Marshell shell integration for PowerShell 7 and Windows PowerShell 5.
# Marshell runs this after PowerShell has loaded your profile. It never edits your profile.
# It marks each prompt and command (OSC 133) and reports the folder (OSC 9;9), so Marshell can
# show whether a command is running and how the last one ended.

if ($global:MARSHELL_SHELL_INTEGRATION -eq 1) { return }
$global:MARSHELL_SHELL_INTEGRATION = 1

$global:__MarshellPrompt = $function:prompt
$global:__MarshellLastId = -1
if ($h = Get-History -Count 1) { $global:__MarshellLastId = $h.Id }

# Emits C once the user has entered a command line. PSReadLine can load after this script,
# so the prompt below wraps it the first time it finds it.
function global:__MarshellWrapReadLine {
    if ($global:__MarshellReadLine -or -not (Test-Path Function:\PSConsoleHostReadLine)) { return }
    $global:__MarshellReadLine = $function:PSConsoleHostReadLine
    function global:PSConsoleHostReadLine {
        $line = & $global:__MarshellReadLine
        if ($line -and $line.Trim()) { [Console]::Write("$([char]27)]133;C$([char]27)\") }
        $line
    }
}

function global:prompt {
    # Read these first: anything run below overwrites them.
    $ok = $global:?
    $code = $global:LASTEXITCODE
    $e = [char]27
    $out = ''
    $h = Get-History -Count 1
    if ($h -and $h.Id -ne $global:__MarshellLastId) {
        $global:__MarshellLastId = $h.Id
        $exit = if ($ok) { 0 } elseif ($code -is [int] -and $code -ne 0) { $code } else { 1 }
        $out += "$e]133;D;$exit$e\"
    } else {
        $out += "$e]133;D$e\"
    }
    $loc = $executionContext.SessionState.Path.CurrentLocation
    if ($loc.Provider.Name -eq 'FileSystem') { $out += "$e]9;9;$($loc.ProviderPath)$e\" }
    $out += "$e]133;A$e\"
    __MarshellWrapReadLine
    $inner = if ($global:__MarshellPrompt) { & $global:__MarshellPrompt } else { "PS $loc> " }
    # Put the exit code back, so the user's own prompt and scripts see the real one.
    $global:LASTEXITCODE = $code
    $out + ($inner -join '')
}

__MarshellWrapReadLine
