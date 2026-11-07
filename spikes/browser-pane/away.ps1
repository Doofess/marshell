# Spike 6q-0: a small window of the spike's own, for Alt+Tab to switch to and back from, so no key ever reaches another
# app. It closes itself after -Minutes; stop it earlier by its PID.
# pwsh -NoProfile -File spikes/browser-pane/away.ps1 [-Minutes 30]
param([int]$Minutes = 30)
Add-Type -AssemblyName System.Windows.Forms, System.Drawing
# Started hidden, Windows applies the hidden state to the process's first ShowWindow. Spend it on the console, so the form
# shows.
Add-Type -Namespace SpikeAway -Name Native -MemberDefinition @'
[DllImport("kernel32.dll")] public static extern IntPtr GetConsoleWindow();
[DllImport("user32.dll")] public static extern bool ShowWindow(IntPtr h, int cmd);
'@
[void][SpikeAway.Native]::ShowWindow([SpikeAway.Native]::GetConsoleWindow(), 0)
$form = [System.Windows.Forms.Form]::new()
$form.Text = 'Spike 6q-0 away window'
$form.StartPosition = 'Manual'
$form.Location = [System.Drawing.Point]::new(40, 40)
$form.Size = [System.Drawing.Size]::new(380, 120)
$form.ShowInTaskbar = $true
$label = [System.Windows.Forms.Label]::new()
$label.Text = 'Marshell spike 6q-0: the Alt+Tab target. It closes on its own.'
$label.Dock = 'Fill'
$label.TextAlign = 'MiddleCenter'
$form.Controls.Add($label)
$timer = [System.Windows.Forms.Timer]::new()
$timer.Interval = $Minutes * 60 * 1000
$timer.Add_Tick({ $form.Close() })
$timer.Start()
[void]$form.ShowDialog()
