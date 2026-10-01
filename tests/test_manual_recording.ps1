param([string]$Executable = (Join-Path $PSScriptRoot '..\src-tauri\target\release\voiceglass.exe'))
$ErrorActionPreference = 'Stop'
$Executable = (Resolve-Path -LiteralPath $Executable).Path
if (Get-Process voiceglass -ErrorAction SilentlyContinue) { throw 'Close the app before this microphone test' }
Add-Type -AssemblyName UIAutomationClient
Add-Type -AssemblyName UIAutomationTypes
Add-Type @'
using System; using System.Runtime.InteropServices;
public class ManualRecordingInput {
[DllImport("user32.dll")] public static extern bool SetCursorPos(int x,int y);
[DllImport("user32.dll")] public static extern void mouse_event(uint flags,uint dx,uint dy,uint data,UIntPtr extra);
[DllImport("user32.dll")] public static extern void keybd_event(byte key,byte scan,uint flags,UIntPtr extra);
}
'@
$app = Start-Process -FilePath $Executable -WorkingDirectory (Split-Path $Executable) -WindowStyle Hidden -PassThru
try {
    Start-Sleep -Seconds 3
    $condition = [System.Windows.Automation.PropertyCondition]::new([System.Windows.Automation.AutomationElement]::ProcessIdProperty,$app.Id)
    $windows = [System.Windows.Automation.AutomationElement]::RootElement.FindAll([System.Windows.Automation.TreeScope]::Children,$condition)
    $window = $windows | Where-Object { $_.Current.Name -eq 'Ferrofluid Voice' } | Select-Object -First 1
    if (!$window) { throw 'Dictation window was not opened' }
    $bounds = $window.Current.BoundingRectangle
    [void][ManualRecordingInput]::SetCursorPos([int]($bounds.X + $bounds.Width * 48 / 431),[int]($bounds.Y + $bounds.Height * 39 / 85))
    function Click-Microphone {
        [ManualRecordingInput]::mouse_event(2,0,0,0,[UIntPtr]::Zero)
        Start-Sleep -Milliseconds 50
        [ManualRecordingInput]::mouse_event(4,0,0,0,[UIntPtr]::Zero)
    }
    $started = Get-Date
    Click-Microphone
    Start-Sleep -Seconds 1
    $files = @(Get-ChildItem (Join-Path $env:APPDATA 'Ferrofluid Voice\recordings') -File | Where-Object { $_.CreationTime -ge $started })
    if ($files.Count -ne 1) { throw 'Manual button did not start exactly one recording' }
    # Ctrl+Q must not take ownership of an already-running manual recording.
    [ManualRecordingInput]::keybd_event(162,0,0,[UIntPtr]::Zero)
    [ManualRecordingInput]::keybd_event(81,0,0,[UIntPtr]::Zero)
    Start-Sleep -Milliseconds 150
    [ManualRecordingInput]::keybd_event(81,0,2,[UIntPtr]::Zero)
    [ManualRecordingInput]::keybd_event(162,0,2,[UIntPtr]::Zero)
    Start-Sleep -Seconds 1
    Click-Microphone
    $finalized = $false
    $stopDeadline = (Get-Date).AddSeconds(5)
    do {
        try {
            $bytes = [IO.File]::ReadAllBytes($files[0].FullName)
            $finalized = $bytes.Length -gt 44 -and [BitConverter]::ToUInt32($bytes,4) -eq $bytes.Length - 8
        } catch [IO.IOException] { } # Recorder may still be closing its stream.
        if (!$finalized) { Start-Sleep -Milliseconds 50 }
    } while (!$finalized -and (Get-Date) -lt $stopDeadline)
    if (!$finalized) { throw 'Manual stop did not finalize WAV' }
    $seenEngine = $false
    $deadline = (Get-Date).AddSeconds(30)
    do {
        $engine = Get-CimInstance Win32_Process | Where-Object { $_.ParentProcessId -eq $app.Id -and $_.Name -like 'whisper*.exe' }
        if ($engine) { $seenEngine = $true }
        if ($seenEngine -and !$engine) { break }
        Start-Sleep -Milliseconds 100
    } while ((Get-Date) -lt $deadline)
    if (!$seenEngine -or $engine) { throw 'Manual recording did not complete Whisper processing' }
    'PASS: physical microphone-button clicks record/finalize WAV and run Whisper; Ctrl+Q does not stop manual recording'
} finally {
    [ManualRecordingInput]::keybd_event(81,0,2,[UIntPtr]::Zero)
    [ManualRecordingInput]::keybd_event(162,0,2,[UIntPtr]::Zero)
    if (!$app.HasExited) { Stop-Process -Id $app.Id }
}
