# Spike 6q-0: what UI Automation sees of the dev app. Prints one JSON line:
#   documents: the Name of every Document under the app's main window (the app's own and the page's);
#   focused:   the focused element's Name and ControlType, and the Document it sits under.
# Another app's element names are never printed.
# pwsh -NoProfile -File spikes/browser-pane/uia.ps1 -AppPid <pid>
param([Parameter(Mandatory)][int]$AppPid)
$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName UIAutomationClient, UIAutomationTypes
$A = [System.Windows.Automation.AutomationElement]
$CT = [System.Windows.Automation.ControlType]

$hwnd = (Get-Process -Id $AppPid).MainWindowHandle
if ($hwnd -eq [IntPtr]::Zero) { throw "process $AppPid has no main window" }
$root = $A::FromHandle($hwnd)
$isDoc = [System.Windows.Automation.PropertyCondition]::new($A::ControlTypeProperty, $CT::Document)
$documents = @($root.FindAll([System.Windows.Automation.TreeScope]::Descendants, $isDoc) | ForEach-Object { $_.Current.Name })

$ours = @('marshell-app', 'msedgewebview2')
$focused = $null
$e = $A::FocusedElement
if ($e) {
    $c = $e.Current
    $proc = (Get-Process -Id $c.ProcessId -ErrorAction SilentlyContinue).ProcessName
    if ($proc -in $ours) {
        $walker = [System.Windows.Automation.TreeWalker]::ControlViewWalker
        $doc = $e
        while ($doc -and $doc.Current.ControlType -ne $CT::Document) { $doc = $walker.GetParent($doc) }
        $focused = [ordered]@{ name = $c.Name; type = $c.ControlType.ProgrammaticName; document = $(if ($doc) { $doc.Current.Name } else { $null }) }
    } else {
        $focused = [ordered]@{ name = '(other app)'; type = $c.ControlType.ProgrammaticName; document = $null }
    }
}
[ordered]@{ documents = $documents; focused = $focused } | ConvertTo-Json -Compress -Depth 4
