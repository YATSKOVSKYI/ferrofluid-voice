param([string]$Executable = (Join-Path $PSScriptRoot '..\src-tauri\target\release\voiceglass.exe'))
$ErrorActionPreference = 'Stop'
$Executable = (Resolve-Path -LiteralPath $Executable).Path
if (Get-Process voiceglass -ErrorAction SilentlyContinue) {
    throw 'Close Ferrofluid Voice before running this isolated lifecycle test.'
}
Add-Type @'
using System;
using System.Runtime.InteropServices;
using System.Text;
public static class InstanceTestNative {
    public delegate bool EnumProc(IntPtr hwnd, IntPtr parameter);
    [DllImport("user32.dll")] public static extern bool EnumWindows(EnumProc callback, IntPtr parameter);
    [DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr hwnd, out uint pid);
    [DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr hwnd);
    [DllImport("user32.dll")] public static extern bool IsIconic(IntPtr hwnd);
    [DllImport("user32.dll")] public static extern bool ShowWindow(IntPtr hwnd, int command);
    [DllImport("user32.dll")] public static extern bool PostMessage(IntPtr hwnd, uint message, IntPtr wparam, IntPtr lparam);
    [DllImport("user32.dll", CharSet=CharSet.Unicode)] public static extern int GetWindowText(IntPtr hwnd, StringBuilder text, int count);
    [DllImport("ntdll.dll")] public static extern int NtSuspendProcess(IntPtr process);
    [DllImport("ntdll.dll")] public static extern int NtResumeProcess(IntPtr process);
    public static IntPtr Window(uint process, string expectedTitle = "Ferrofluid Voice Library") {
        IntPtr result = IntPtr.Zero;
        EnumWindows((hwnd, parameter) => {
            uint pid; GetWindowThreadProcessId(hwnd, out pid);
            var title = new StringBuilder(256); GetWindowText(hwnd, title, title.Capacity);
            if (pid == process && IsWindowVisible(hwnd) && title.ToString() == expectedTitle) { result = hwnd; return false; }
            return true;
        }, IntPtr.Zero);
        return result;
    }
}
'@
function Launch-App {
    param([switch]$Widget)
    $launchOptions = @{ FilePath=$Executable; WorkingDirectory=(Split-Path $Executable); WindowStyle='Hidden'; PassThru=$true }
    if (!$Widget) { $launchOptions.ArgumentList = '--library' }
    Start-Process @launchOptions
}
function Assert-One {
    $running = @(Get-Process voiceglass -ErrorAction SilentlyContinue)
    if ($running.Count -ne 1) { throw "Expected one owner, found $($running.Count)" }
    return $running[0]
}
$owner = $null
$suspended = $false
try {
    # Burst before the first owner's activation receiver has finished startup.
    $launches = @(1..20 | ForEach-Object { Launch-App })
    Start-Sleep -Seconds 12
    $owner = Assert-One
    foreach ($process in $launches) {
        if ($process.Id -ne $owner.Id -and !$process.WaitForExit(1000)) { throw 'Duplicate did not exit' }
    }
    $window = [InstanceTestNative]::Window($owner.Id)
    if ($window -eq [IntPtr]::Zero) { throw 'Owner has no visible window' }
    'PASS: 20 simultaneous cold launches produce one owner and visible UI'

    [void][InstanceTestNative]::ShowWindow($window, 0)
    $repeat = Launch-App -Widget
    if (!$repeat.WaitForExit(4000)) { throw 'Repeat launch hung' }
    Start-Sleep -Seconds 2
    if (!( [InstanceTestNative]::IsWindowVisible($window))) { throw 'Hidden window was not restored' }
    if ((Assert-One).Id -ne $owner.Id) { throw 'Owner changed on activation' }
    [void][InstanceTestNative]::ShowWindow($window, 6)
    $repeat = Launch-App
    if (!$repeat.WaitForExit(4000)) { throw 'Minimized activation hung' }
    Start-Sleep -Seconds 2
    if ([InstanceTestNative]::IsIconic($window)) { throw 'Minimized window was not restored' }
    'PASS: repeated launch restores hidden and minimized window without replacing owner'

    # Suspend only the test-owned process, then prove bounded duplicate lifetime.
    if ([InstanceTestNative]::NtSuspendProcess($owner.Handle) -ne 0) { throw 'Suspend failed' }
    $suspended = $true
    $timer = [Diagnostics.Stopwatch]::StartNew()
    $repeat = Launch-App
    if (!$repeat.WaitForExit(5000)) { throw 'Duplicate waited indefinitely for frozen owner' }
    if ($repeat.ExitCode -ne 2) { throw "Expected timeout exit 2, got $($repeat.ExitCode)" }
    if ((Assert-One).Id -ne $owner.Id) { throw 'Frozen owner caused another instance' }
    [void][InstanceTestNative]::NtResumeProcess($owner.Handle)
    $suspended = $false
    "PASS: frozen-owner duplicate exits in $([math]::Round($timer.Elapsed.TotalSeconds, 2)) seconds"

    Stop-Process -Id $owner.Id -Force
    $owner.WaitForExit()
    $owner = Launch-App -Widget
    Start-Sleep -Seconds 5
    if ((Assert-One).Id -ne $owner.Id) { throw 'Crash recovery failed' }
    $widget = [InstanceTestNative]::Window($owner.Id, 'Ferrofluid Voice')
    if ($widget -eq [IntPtr]::Zero) { throw 'Recovered default-launch widget is not visible' }
    'PASS: kernel lock is released after forced termination; fresh launch succeeds'

    $repeat = Launch-App
    if (!$repeat.WaitForExit(4000)) { throw 'Opening Library from widget hung' }
    Start-Sleep -Seconds 2
    if ([InstanceTestNative]::Window($owner.Id) -eq [IntPtr]::Zero) { throw 'Library did not open' }
    [void][InstanceTestNative]::PostMessage($widget, 0x10, [IntPtr]::Zero, [IntPtr]::Zero)
    if (!$owner.WaitForExit(5000)) { throw 'Closing widget with Library open left a background owner' }
    if (Get-Process voiceglass -ErrorAction SilentlyContinue) { throw 'Graceful exit left app processes' }
    'PASS: default launch shows widget; closing widget exits even with Library open'
} finally {
    if ($owner -and !$owner.HasExited) {
        if ($suspended) { [void][InstanceTestNative]::NtResumeProcess($owner.Handle) }
        Stop-Process -Id $owner.Id -Force
    }
}
