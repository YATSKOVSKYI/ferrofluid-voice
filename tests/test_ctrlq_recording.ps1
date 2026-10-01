param([string]$Executable = (Join-Path $PSScriptRoot '..\src-tauri\target\release\voiceglass.exe'))
$ErrorActionPreference = 'Stop'
$Executable = (Resolve-Path -LiteralPath $Executable).Path
if (Get-Process voiceglass -ErrorAction SilentlyContinue) { throw 'Close the app before this microphone integration test' }
$config = Get-Content (Join-Path $env:APPDATA 'Ferrofluid Voice\settings.json') -Raw | ConvertFrom-Json
if ($config.hotkeyType -ne 'chord_17+81' -or $config.autoSubmit) { throw 'Configure Ctrl+Q and disable auto-submit before testing' }
if (!(Test-Path -LiteralPath $config.modelPath)) { throw 'Select a downloaded Whisper model before testing' }
$outputDir = Join-Path (Split-Path $PSScriptRoot) '.cache\ctrlq-integration'
New-Item -ItemType Directory -Path $outputDir -Force | Out-Null
Add-Type @'
using System;
using System.Runtime.InteropServices;
public class CtrlQIntegration {
[DllImport("user32.dll")] public static extern void keybd_event(byte key,byte scan,uint flags,UIntPtr extra);
[DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr hwnd);
}
'@
$app = $null
try {
    $stdout = Join-Path $outputDir 'stdout.log'
    $stderr = Join-Path $outputDir 'stderr.log'
    $app = Start-Process -FilePath $Executable -WorkingDirectory (Split-Path $Executable) -WindowStyle Hidden -PassThru -RedirectStandardOutput $stdout -RedirectStandardError $stderr
    Start-Sleep -Seconds 3
    [void][CtrlQIntegration]::SetForegroundWindow($app.MainWindowHandle)
    foreach ($firstRelease in @(81,162)) {
        $started = Get-Date
        try {
            [CtrlQIntegration]::keybd_event(162,0,0,[UIntPtr]::Zero)
            [CtrlQIntegration]::keybd_event(81,0,0,[UIntPtr]::Zero)
            Start-Sleep -Seconds 1
            [CtrlQIntegration]::keybd_event($firstRelease,0,2,[UIntPtr]::Zero)
            Start-Sleep -Milliseconds 500
            $wav = Get-ChildItem (Join-Path $env:APPDATA 'Ferrofluid Voice\recordings') -File |
                Where-Object { $_.CreationTime -ge $started } | Sort-Object CreationTime -Descending | Select-Object -First 1
            if (!$wav) { throw 'Ctrl+Q did not create audio' }
            $data = [IO.File]::ReadAllBytes($wav.FullName)
            if ($data.Length -le 44 -or [Text.Encoding]::ASCII.GetString($data,0,4) -ne 'RIFF' -or [BitConverter]::ToUInt32($data,4) -ne ($data.Length - 8)) {
                throw 'Release did not finalize the WAV recording'
            }
            # The other key is still held: finalized RIFF proves release of either
            # key stopped the microphone, rather than waiting for both releases.
        } finally {
            [CtrlQIntegration]::keybd_event(81,0,2,[UIntPtr]::Zero)
            [CtrlQIntegration]::keybd_event(162,0,2,[UIntPtr]::Zero)
        }
        $seenEngine = $false
        $deadline = (Get-Date).AddSeconds(30)
        do {
            $engine = Get-CimInstance Win32_Process | Where-Object { $_.ParentProcessId -eq $app.Id -and $_.Name -like 'whisper*.exe' }
            if ($engine) { $seenEngine = $true }
            if ($seenEngine -and !$engine) { break }
            Start-Sleep -Milliseconds 100
        } while ((Get-Date) -lt $deadline)
        if (!$seenEngine) { throw 'Automatic Whisper processing was not observed' }
        if ($engine) { throw 'Whisper did not finish within the integration-test deadline' }
        if (!(Select-String -LiteralPath $stdout -Pattern 'Calling finishHotkeyRecording\(true\)' -Quiet)) { throw 'Widget did not handle release' }
        Start-Sleep -Seconds 1
        "PASS: Ctrl+Q records, releasing VK=$firstRelease finalizes WAV while the other key remains held, and Whisper runs"
    }
} finally {
    [CtrlQIntegration]::keybd_event(81,0,2,[UIntPtr]::Zero)
    [CtrlQIntegration]::keybd_event(162,0,2,[UIntPtr]::Zero)
    if ($app -and !$app.HasExited) { Stop-Process -Id $app.Id }
}
