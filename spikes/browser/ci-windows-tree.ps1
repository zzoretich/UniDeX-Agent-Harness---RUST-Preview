# CI only (Windows): lists the trial's process tree (every process started under spike-smoke.exe)
# with each process's role from its command line, its integrity level, whether its token is
# restricted, whether it runs in an AppContainer and in a job, and the listening TCP sockets any
# of them own (lines starting "listening:"). Chromium's Windows sandbox runs renderers with a
# restricted token at Untrusted integrity (0x0000) inside a job. Writes a verdict for the workflow
# to $env:RUNNER_TEMP\windows-sandbox-verdict.txt: "ok: ..." when at least one renderer exists
# and every renderer runs at Untrusted or Low integrity with a restricted token, "fail: ..."
# otherwise. Read-only: it opens processes and tokens for query only.
$ErrorActionPreference = 'Continue'

Add-Type -TypeDefinition @"
using System;
using System.Runtime.InteropServices;
public static class SandboxProbe {
    [DllImport("kernel32.dll", SetLastError = true)]
    static extern IntPtr OpenProcess(uint access, bool inherit, int pid);
    [DllImport("kernel32.dll", SetLastError = true)]
    static extern bool CloseHandle(IntPtr handle);
    [DllImport("kernel32.dll", SetLastError = true)]
    static extern bool IsProcessInJob(IntPtr process, IntPtr job, out bool result);
    [DllImport("advapi32.dll", SetLastError = true)]
    static extern bool OpenProcessToken(IntPtr process, uint access, out IntPtr token);
    [DllImport("advapi32.dll", SetLastError = true)]
    static extern bool GetTokenInformation(IntPtr token, int infoClass, IntPtr buffer, int length, out int returned);
    [DllImport("advapi32.dll", SetLastError = true)]
    static extern bool IsTokenRestricted(IntPtr token);
    [DllImport("advapi32.dll")]
    static extern IntPtr GetSidSubAuthorityCount(IntPtr sid);
    [DllImport("advapi32.dll")]
    static extern IntPtr GetSidSubAuthority(IntPtr sid, uint index);

    const uint PROCESS_QUERY_LIMITED_INFORMATION = 0x1000;
    const uint TOKEN_QUERY = 0x0008;
    const int TokenIntegrityLevel = 25;
    const int TokenIsAppContainer = 29;

    // Returns "integrity=0x.... restricted=... appcontainer=... job=..." or what failed.
    public static string Describe(int pid) {
        IntPtr process = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid);
        if (process == IntPtr.Zero) return "open-process-failed=" + Marshal.GetLastWin32Error();
        string result = "";
        try {
            bool inJob;
            if (IsProcessInJob(process, IntPtr.Zero, out inJob)) result += "job=" + inJob + " ";
            IntPtr token;
            if (!OpenProcessToken(process, TOKEN_QUERY, out token))
                return result + "open-token-failed=" + Marshal.GetLastWin32Error();
            try {
                int length;
                GetTokenInformation(token, TokenIntegrityLevel, IntPtr.Zero, 0, out length);
                if (length > 0) {
                    IntPtr buffer = Marshal.AllocHGlobal(length);
                    try {
                        if (GetTokenInformation(token, TokenIntegrityLevel, buffer, length, out length)) {
                            IntPtr sid = Marshal.ReadIntPtr(buffer);
                            int count = Marshal.ReadByte(GetSidSubAuthorityCount(sid));
                            int rid = Marshal.ReadInt32(GetSidSubAuthority(sid, (uint)(count - 1)));
                            result += "integrity=0x" + rid.ToString("x4") + " ";
                        }
                    } finally { Marshal.FreeHGlobal(buffer); }
                }
                result += "restricted=" + IsTokenRestricted(token) + " ";
                IntPtr flag = Marshal.AllocHGlobal(4);
                try {
                    int returned;
                    if (GetTokenInformation(token, TokenIsAppContainer, flag, 4, out returned))
                        result += "appcontainer=" + (Marshal.ReadInt32(flag) != 0) + " ";
                } finally { Marshal.FreeHGlobal(flag); }
            } finally { CloseHandle(token); }
        } finally { CloseHandle(process); }
        return result.Trim();
    }
}
"@

$all = @(Get-CimInstance Win32_Process | Select-Object ProcessId, ParentProcessId, Name, CommandLine)
# Keyed by the parent's id as text, so the number types of the two properties never matter.
$children = @{}
foreach ($p in $all) {
    $key = [string]$p.ParentProcessId
    if (-not $children.ContainsKey($key)) { $children[$key] = New-Object System.Collections.ArrayList }
    [void]$children[$key].Add($p)
}
$queue = New-Object System.Collections.Queue
foreach ($p in $all) {
    if ($p.Name -eq 'spike-smoke.exe') { $queue.Enqueue([string]$p.ProcessId) }
}
$tree = New-Object System.Collections.ArrayList
while ($queue.Count -gt 0) {
    $key = $queue.Dequeue()
    if (-not $children.ContainsKey($key)) { continue }
    foreach ($c in $children[$key]) {
        [void]$tree.Add($c)
        $queue.Enqueue([string]$c.ProcessId)
    }
}

function Switch-Value($line, $name) {
    if ($line -match ("(^|\s)" + [regex]::Escape($name) + "([^\s`"]+)")) { return $Matches[2] }
    return $null
}

$renderers = 0
$sandboxedRenderers = 0
foreach ($p in $tree) {
    $line = [string]$p.CommandLine
    $kind = Switch-Value $line '--type='
    if ($null -eq $kind) {
        $kind = if ($p.Name -eq 'spike-cef-helper.exe') { 'browser' } else { 'other' }
    }
    $sub = Switch-Value $line '--utility-sub-type='
    if ($null -ne $sub) { $kind = "${kind}:$sub" }
    $sandboxType = Switch-Value $line '--service-sandbox-type='
    $state = [SandboxProbe]::Describe([int]$p.ProcessId)
    $typeText = if ($null -ne $sandboxType) { " sandbox-type=$sandboxType" } else { "" }
    Write-Output ("process {0} parent {1} {2} {3}{4} {5}" -f $p.ProcessId, $p.ParentProcessId, $p.Name, $kind, $typeText, $state)
    if ($kind -eq 'renderer') {
        $renderers++
        if ($state -match 'integrity=0x(0000|1000)' -and $state -match 'restricted=True') { $sandboxedRenderers++ }
    }
}
Write-Output ("processes in the tree: {0}" -f $tree.Count)

$ids = @($tree | ForEach-Object { [int]$_.ProcessId })
foreach ($s in @(Get-NetTCPConnection -State Listen -ErrorAction SilentlyContinue)) {
    if ($ids -contains [int]$s.OwningProcess) {
        Write-Output ("listening: pid={0} address={1} port={2}" -f $s.OwningProcess, $s.LocalAddress, $s.LocalPort)
    }
}

if ($renderers -gt 0 -and $sandboxedRenderers -eq $renderers) {
    $verdict = "ok: $renderers of $renderers renderers at Untrusted or Low integrity with a restricted token"
} else {
    $verdict = "fail: $sandboxedRenderers of $renderers renderers at Untrusted or Low integrity with a restricted token"
}
Write-Output "sandbox verdict: $verdict"
Set-Content -Path (Join-Path $env:RUNNER_TEMP 'windows-sandbox-verdict.txt') -Value $verdict
