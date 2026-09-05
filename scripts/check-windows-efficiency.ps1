[CmdletBinding()]
param([Parameter(Mandatory)][int]$ProcessId)
$ErrorActionPreference = 'Stop'
if (-not ('WindowsEfficiencyProbe' -as [type])) {
    Add-Type @"
using System;
using System.Runtime.InteropServices;
public static class WindowsEfficiencyProbe {
    [StructLayout(LayoutKind.Sequential)] public struct PowerState { public uint Version, ControlMask, StateMask; }
    [DllImport("kernel32.dll", SetLastError=true)] public static extern IntPtr OpenProcess(uint access, bool inherit, int pid);
    [DllImport("kernel32.dll", SetLastError=true)] public static extern bool GetProcessInformation(IntPtr process, int kind, ref PowerState value, uint size);
    [DllImport("kernel32.dll", SetLastError=true)] public static extern uint GetPriorityClass(IntPtr process);
    [DllImport("kernel32.dll")] public static extern bool CloseHandle(IntPtr handle);
}
"@
}
$process = Get-Process -Id $ProcessId
$handle = [WindowsEfficiencyProbe]::OpenProcess(0x1000, $false, $ProcessId)
if ($handle -eq [IntPtr]::Zero) { throw 'Unable to open the requested process for read-only inspection' }
try {
    $state = New-Object WindowsEfficiencyProbe+PowerState
    $state.Version = 1
    if (-not [WindowsEfficiencyProbe]::GetProcessInformation($handle, 4, [ref]$state, 12)) { throw 'Windows did not return process power-throttling state' }
    $priority = [WindowsEfficiencyProbe]::GetPriorityClass($handle)
    [pscustomobject]@{ pid=$ProcessId; path=$process.Path; eco_qos=(($state.ControlMask -band 1) -ne 0 -and ($state.StateMask -band 1) -ne 0); idle_priority=($priority -eq 0x40); priority_class=$priority; control_mask=$state.ControlMask; state_mask=$state.StateMask } | ConvertTo-Json
} finally { [WindowsEfficiencyProbe]::CloseHandle($handle) | Out-Null }
