$ErrorActionPreference = 'Stop'
$projectRoot = Split-Path -Parent $PSScriptRoot
$cargoBin = Join-Path $env:USERPROFILE '.cargo\bin'
$env:Path = "$cargoBin;$env:Path"

Push-Location $projectRoot
try {
    node scripts\prepare-build.mjs
    if ($LASTEXITCODE -ne 0) { throw 'Version preparation failed.' }
    node node_modules\vite\bin\vite.js build
    if ($LASTEXITCODE -ne 0) { throw 'Frontend build failed.' }
    node node_modules\@tauri-apps\cli\tauri.js build --no-bundle --config scripts\portable-build-config.json
    if ($LASTEXITCODE -ne 0) { throw 'Tauri build failed.' }

    $artifactsRoot = [System.IO.Path]::GetFullPath((Join-Path $projectRoot 'artifacts\portable'))
    $portableRoot = [System.IO.Path]::GetFullPath((Join-Path $artifactsRoot 'MoveMgr-win-x64'))
    if (-not $portableRoot.StartsWith($artifactsRoot + [System.IO.Path]::DirectorySeparatorChar, [System.StringComparison]::OrdinalIgnoreCase)) {
        throw 'Portable output path escaped the artifacts directory.'
    }
    New-Item -ItemType Directory -Path $portableRoot -Force | Out-Null
    $builtExe = Join-Path $projectRoot 'src-tauri\target\release\MoveMgr.exe'
    $mainExe = Join-Path $portableRoot 'MoveMgr.exe'
    $launchExe = $mainExe
    try {
        Copy-Item -LiteralPath $builtExe -Destination $mainExe -Force
    } catch {
        $running = @(Get-Process -Name 'MoveMgr' -ErrorAction SilentlyContinue | Where-Object {
            $_.Path -eq $mainExe
        }).Count -gt 0
        if (-not $running) { throw }
        $versionInfo = Get-Content -LiteralPath (Join-Path $projectRoot 'version.json') -Encoding UTF8 | ConvertFrom-Json
        $launchExe = Join-Path $portableRoot "MoveMgr-$($versionInfo.version).exe"
        Copy-Item -LiteralPath $builtExe -Destination $launchExe -Force
        Write-Warning 'MoveMgr.exe is running. Close the old app, then launch the versioned EXE below.'
    }
    Copy-Item -LiteralPath (Join-Path $projectRoot 'portable\PORTABLE_README.txt') -Destination (Join-Path $portableRoot 'PORTABLE_README.txt') -Force
    Write-Output "Portable build: $launchExe"
} finally {
    Pop-Location
}
