# Marshell shell integration for PowerShell 7 and Windows PowerShell 5.
# Marshell runs this after PowerShell has loaded your profile. It never edits your profile.
# It marks each prompt and command (OSC 133) and reports the folder (OSC 9;9), so Marshell can
# show whether a command is running and how the last one ended.
# Your profile may turn on strict mode: the reads here don't fail under it, and each function
# turns it off for itself only (a function's own scope), so your setting stays as it is.

if ((Get-Variable MARSHELL_SHELL_INTEGRATION -Scope Global -ValueOnly -ErrorAction Ignore) -eq 1) { return }
$global:MARSHELL_SHELL_INTEGRATION = 1

$global:__MarshellPrompt = $function:prompt
$global:__MarshellReadLine = $null
$global:__MarshellLastId = -1
if ($h = Get-History -Count 1) { $global:__MarshellLastId = $h.Id }
$global:__MarshellLastError = if ($Error.Count -gt 0) { $Error[0] } else { $null }

# Emits C once the user has entered a command line. PSReadLine can load after this script,
# so the prompt below wraps it the first time it finds it.
function global:__MarshellWrapReadLine {
    Set-StrictMode -Off
    if ($global:__MarshellReadLine -or -not (Test-Path Function:\PSConsoleHostReadLine)) { return }
    $global:__MarshellReadLine = $function:PSConsoleHostReadLine
    function global:PSConsoleHostReadLine {
        Set-StrictMode -Off
        $line = & $global:__MarshellReadLine
        if ($line -and $line.Trim()) {
            # Report the typed line first: a backslash is \\, a semicolon is \x3b, a control char is \xHH.
            # [regex]::Replace with a scriptblock works on Windows PowerShell 5.1 as well as 7.
            $esc = [regex]::Replace(($line -replace '\\', '\\' -replace ';', '\x3b'), '[\x00-\x1f]', { param($m) '\x{0:x2}' -f [int][char]$m.Value })
            [Console]::Write("$([char]27)]633;E;$esc$([char]27)\$([char]27)]133;C$([char]27)\")
        }
        $line
    }
}

function global:prompt {
    # Read these first: anything run below overwrites them.
    $ok = $global:?
    Set-StrictMode -Off
    $code = $global:LASTEXITCODE
    $e = [char]27
    $out = ''
    $h = Get-History -Count 1
    if ($h -and $h.Id -ne $global:__MarshellLastId) {
        $global:__MarshellLastId = $h.Id
        # A failed cmdlet adds to $Error; a native command that exits non-zero doesn't, and sets $LASTEXITCODE.
        # So the code is only the native one when no new error came, never a code left over from earlier.
        $newError = $Error.Count -gt 0 -and -not [object]::ReferenceEquals($Error[0], $global:__MarshellLastError)
        $exit = if ($ok) { 0 } elseif (-not $newError -and $code -is [int] -and $code -ne 0) { $code } else { 1 }
        $out += "$e]133;D;$exit$e\"
    } else {
        $out += "$e]133;D$e\"
    }
    $loc = $executionContext.SessionState.Path.CurrentLocation
    if ($loc.Provider.Name -eq 'FileSystem') { $out += "$e]9;9;$($loc.ProviderPath)$e\" }
    $out += "$e]133;A$e\"
    __MarshellWrapReadLine
    $inner = $global:__MarshellPrompt
    if (-not $inner) { $inner = { "PS $($executionContext.SessionState.Path.CurrentLocation)> " } }
    # The user's own prompt (oh-my-posh, starship) reads $LASTEXITCODE and $?: give it the real ones.
    # Write-Error with Ignore records nothing and only sets $? to false, as the failed command did.
    if ($null -ne $code) { $global:LASTEXITCODE = $code }
    if (-not $ok) { Write-Error 'failure' -ErrorAction Ignore }
    $text = & $inner
    if ($null -ne $code) { $global:LASTEXITCODE = $code }
    $global:__MarshellLastError = if ($Error.Count -gt 0) { $Error[0] } else { $null }
    $out + ($text -join '')
}

__MarshellWrapReadLine
