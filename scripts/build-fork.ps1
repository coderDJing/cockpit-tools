$ErrorActionPreference = 'Stop'
if (-not $env:TAURI_SIGNING_PRIVATE_KEY) {
    throw 'Set TAURI_SIGNING_PRIVATE_KEY to your private signing-key file before building.'
}
Push-Location (Split-Path -Parent $PSScriptRoot)
try {
    $profilePath = Join-Path $PSScriptRoot 'fork-release.toml'
    $runnerPath = Join-Path $PSScriptRoot 'cargo-fork-windows.cmd'
    & npm.cmd run tauri build -- --bundles nsis --runner $runnerPath -- --config $profilePath
    if ($LASTEXITCODE -ne 0) { throw "Fork build failed ($LASTEXITCODE)" }
} finally {
    Pop-Location
}
