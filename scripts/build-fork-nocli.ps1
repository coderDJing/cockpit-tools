# Fork release build WITHOUT the Tauri CLI.
#
# Why this exists: `npm run tauri build` needs to run build scripts that shell
# out to reg.exe (vswhom-sys), which is blocked in restricted environments.
# This path drives cargo directly and produces the same executable.
#
# The one thing that must not be dropped: --features tauri/custom-protocol.
# Tauri decides at compile time whether the webview loads the embedded assets
# or devUrl (tauri-macros/src/context.rs: `dev: cfg!(not(feature = "custom-protocol"))`).
# Without the feature the binary believes it is a dev build, ignores
# `frontendDist`, and opens http://localhost:1420 -> ERR_CONNECTION_REFUSED.
#
# -C link-arg=/DEBUG:NONE keeps link.exe under the MSVC PDB limit (LNK1318).
$ErrorActionPreference = 'Stop'
Push-Location (Split-Path -Parent $PSScriptRoot)
try {
    # Same frontend step the Tauri CLI would run (see tauri.conf.json beforeBuildCommand).
    & cmd.exe /c 'node scripts/prepare-tauri.cjs && npm run build'
    if ($LASTEXITCODE -ne 0) { throw "frontend build failed ($LASTEXITCODE)" }

    & cargo.exe rustc --release -p cockpit-tools --bin cockpit-tools `
        --features tauri/custom-protocol `
        -- -C link-arg=/DEBUG:NONE
    if ($LASTEXITCODE -ne 0) { throw "cargo rustc failed ($LASTEXITCODE)" }

    $exe = Join-Path $PSScriptRoot '..\target\release\cockpit-tools.exe'
    $item = Get-Item $exe
    "{0}  {1:N0} bytes  FileVersion={2}" -f $item.FullName, $item.Length, $item.VersionInfo.FileVersion
} finally {
    Pop-Location
}
