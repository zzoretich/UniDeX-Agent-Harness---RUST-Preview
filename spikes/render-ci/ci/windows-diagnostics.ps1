# Prints what the Windows runner offers for rendering without a GPU: the session (interactive or not), the display
# adapters, the screens, and DirectX's view of the adapters. Diagnostics only; nothing here fails the job.
$ErrorActionPreference = 'Continue'
Write-Host "----- OS"
[System.Environment]::OSVersion.VersionString
Write-Host "----- session"
Write-Host ("UserInteractive: " + [Environment]::UserInteractive)
Write-Host ("SessionId: " + (Get-Process -Id $PID).SessionId)
Write-Host ("Console session (WTSGetActiveConsoleSessionId):")
Add-Type @'
using System.Runtime.InteropServices;
public static class Wts {
    [DllImport("kernel32.dll")] public static extern uint WTSGetActiveConsoleSessionId();
    [DllImport("user32.dll")] public static extern int GetSystemMetrics(int index);
}
'@
[Wts]::WTSGetActiveConsoleSessionId()
Write-Host ("Screen (SM_CXSCREEN x SM_CYSCREEN): " + [Wts]::GetSystemMetrics(0) + " x " + [Wts]::GetSystemMetrics(1))
Write-Host ("Remote session (SM_REMOTESESSION): " + [Wts]::GetSystemMetrics(0x1000))
Write-Host "----- video controllers"
Get-CimInstance Win32_VideoController | Format-List Name, AdapterCompatibility, DriverVersion, VideoProcessor, CurrentHorizontalResolution, CurrentVerticalResolution, Status
Write-Host "----- screens"
Add-Type -AssemblyName System.Windows.Forms
[System.Windows.Forms.Screen]::AllScreens | Format-List DeviceName, Bounds, Primary
Write-Host "----- dxdiag (selected lines)"
$dx = Join-Path $env:RUNNER_TEMP 'dxdiag.txt'
$p = Start-Process -FilePath dxdiag.exe -ArgumentList '/t', $dx -PassThru
if (-not $p.WaitForExit(90000)) { Write-Host "dxdiag did not finish in 90 s"; $p.Kill() }
if (Test-Path $dx) {
    Get-Content $dx | Select-String -Pattern 'Card name|Manufacturer:|Chip type|Driver Version|Driver Model|Feature Levels|DDI Version|Display Memory|Current Mode|DirectX Version|Operating System' | ForEach-Object { $_.Line.Trim() }
    Remove-Item $dx
}
else { Write-Host "(no dxdiag output)" }
exit 0
