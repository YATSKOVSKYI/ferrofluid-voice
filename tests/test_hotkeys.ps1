$ErrorActionPreference = 'Stop'
if (Get-Process voiceglass -ErrorAction SilentlyContinue) {
    throw 'Close Ferrofluid Voice before testing its real Windows hooks.'
}
$projectRoot = Split-Path $PSScriptRoot
Push-Location $projectRoot
try {
    $artifacts = & cargo test --manifest-path src-tauri/Cargo.toml --lib --no-run --message-format=json
    if ($LASTEXITCODE -ne 0) { throw 'Test build failed' }
    $testExecutable = $artifacts | ForEach-Object {
        $artifact = $_ | ConvertFrom-Json
        if ($artifact.reason -eq 'compiler-artifact' -and $artifact.profile.test -and $artifact.executable) { $artifact.executable }
    } | Select-Object -Last 1
    # Tauri embeds Common Controls v6 in the app, but Cargo's unit-test executable
    # needs its own activation manifest when native UI code is linked by the hook test.
    $manifestTool = Get-ChildItem 'C:\Program Files (x86)\Windows Kits\10\bin' -Filter mt.exe -Recurse |
        Where-Object { $_.Directory.Name -eq 'x64' } | Sort-Object FullName | Select-Object -Last 1
    if (!$manifestTool) { throw 'Windows SDK mt.exe was not found' }
    $manifestFile = Join-Path (Split-Path $testExecutable) 'hotkey-test.manifest'
    @'
<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<assembly xmlns="urn:schemas-microsoft-com:asm.v1" manifestVersion="1.0">
<dependency><dependentAssembly><assemblyIdentity type="win32" name="Microsoft.Windows.Common-Controls" version="6.0.0.0" processorArchitecture="*" publicKeyToken="6595b64144ccf1df" language="*"/></dependentAssembly></dependency>
</assembly>
'@ | Set-Content -LiteralPath $manifestFile -Encoding utf8
    & $manifestTool.FullName -nologo -manifest $manifestFile "-outputresource:$testExecutable;#1"
    if ($LASTEXITCODE -ne 0) { throw 'Test manifest embedding failed' }
    & $testExecutable hotkey --nocapture
    if ($LASTEXITCODE -ne 0) { throw 'Hotkey regression failed' }
} finally { Pop-Location }
