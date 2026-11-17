# Captures the screen pixels under the trial's window on a Windows runner and saves them as PNG.
#
#   pwsh -File ci/capture-window.ps1 -Scenario out/<label>/scenario.json -Out out/<label>/display.png
#
# Only the window's own rectangle is copied (from `window.bounds_logical` and `window.scale_factor` in the scenario's
# JSON), so the artifact never holds anything else from the runner's desktop. Uses .NET's Graphics.CopyFromScreen,
# which needs an interactive desktop: on a non-interactive session it fails, and that failure is the finding.
param(
    [Parameter(Mandatory = $true)][string]$Scenario,
    [Parameter(Mandatory = $true)][string]$Out
)
$ErrorActionPreference = 'Stop'

Add-Type -AssemblyName System.Drawing
Add-Type @'
using System.Runtime.InteropServices;
public static class Dpi {
    [DllImport("user32.dll")] public static extern bool SetProcessDPIAware();
}
'@
[void][Dpi]::SetProcessDPIAware()

$s = Get-Content -Raw -Path $Scenario | ConvertFrom-Json
$b = $s.window.bounds_logical
$scale = [double]$s.window.scale_factor
$x = [int][math]::Round($b[0] * $scale)
$y = [int][math]::Round($b[1] * $scale)
$w = [int][math]::Round($b[2] * $scale)
$h = [int][math]::Round($b[3] * $scale)
Write-Host "capturing the window's rectangle: x=$x y=$y w=$w h=$h (scale $scale)"
if ($w -le 0 -or $h -le 0) { throw "empty window rectangle" }

$bmp = New-Object System.Drawing.Bitmap $w, $h
$g = [System.Drawing.Graphics]::FromImage($bmp)
try {
    $g.CopyFromScreen($x, $y, 0, 0, (New-Object System.Drawing.Size $w, $h))
    $bmp.Save($Out, [System.Drawing.Imaging.ImageFormat]::Png)
    Write-Host "saved $Out"
}
finally {
    $g.Dispose()
    $bmp.Dispose()
}
