$taskExecutable = Join-Path $PSScriptRoot 'src-tauri\target\release\voiceglass.exe'
if (-not (Test-Path -LiteralPath $taskExecutable)) {
    throw 'Build the app first: npm run tauri:build -- --no-bundle'
}
Start-Process -FilePath $taskExecutable -ArgumentList '--library' -WorkingDirectory $PSScriptRoot -WindowStyle Hidden
