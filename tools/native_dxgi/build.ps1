# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (C) 2026 Artificial Sweetener and contributors
param([Parameter(Mandatory)][string]$ArtifactRoot, [switch]$RunTests)
$ErrorActionPreference = 'Stop'
if (-not [IO.Path]::IsPathFullyQualified($ArtifactRoot)) { throw 'ArtifactRoot must be absolute' }
$artifactRoot = [IO.Path]::GetFullPath($ArtifactRoot)
if ([IO.Path]::GetPathRoot($artifactRoot) -ine 'D:\') { throw 'ArtifactRoot must resolve to drive D:' }
# Resolve every existing ancestor before creating any artifact or temporary directory.
$resolved = [IO.Path]::GetPathRoot($artifactRoot)
foreach ($component in $artifactRoot.Substring($resolved.Length).Split('\', [StringSplitOptions]::RemoveEmptyEntries)) {
    $resolved = Join-Path $resolved $component
    if (Test-Path -LiteralPath $resolved) {
        $entry = Get-Item -LiteralPath $resolved -Force
        if (-not $entry.PSIsContainer) { throw 'ArtifactRoot ancestor must be a directory' }
        if ($entry.Attributes -band [IO.FileAttributes]::ReparsePoint) {
            $target = $entry.ResolveLinkTarget($true)
            if ($null -eq $target) { throw 'ArtifactRoot reparse target could not be resolved' }
            $resolved = $target.FullName
        }
        if ([IO.Path]::GetPathRoot($resolved) -ine 'D:\') { throw 'ArtifactRoot reparse target leaves drive D:' }
    }
}
$artifactRoot = $resolved
$buildRoot = Join-Path $artifactRoot ('build-' + [Guid]::NewGuid().ToString('N'))
$tempRoot = Join-Path $artifactRoot 'temp'
New-Item -ItemType Directory -Force -Path $buildRoot, $tempRoot | Out-Null
$env:TEMP = $tempRoot
$env:TMP = $tempRoot
$vswhere = Join-Path ${env:ProgramFiles(x86)} 'Microsoft Visual Studio\Installer\vswhere.exe'
$installation = & $vswhere -products Microsoft.VisualStudio.Product.Community -version '[17.13,17.14)' -format json | ConvertFrom-Json
$selected = @($installation | Where-Object { $_.catalog.productDisplayVersion -eq '17.13.6' })
if ($selected.Count -ne 1) {
    throw 'Expected selected VS2022 Community 17.13.6 installation'
}
$vsRoot = $selected[0].installationPath
Import-Module (Join-Path $vsRoot 'Common7\Tools\Microsoft.VisualStudio.DevShell.dll')
Enter-VsDevShell -VsInstallPath $vsRoot -SkipAutomaticLocation -DevCmdArguments '-arch=x64 -host_arch=x64 -vcvars_ver=14.43 -winsdk=10.0.26100.0'
if ($env:VCToolsVersion -ne '14.43.34808' -or $env:WindowsSDKVersion.TrimEnd('\') -ne '10.0.26100.0') {
    throw 'Developer shell did not select pinned VC14.43.34808/SDK10.0.26100.0'
}
$compiler = Join-Path $vsRoot 'VC\Tools\MSVC\14.43.34808\bin\Hostx64\x64\cl.exe'
$sourceRoot = $PSScriptRoot
function Get-NativeSourceHashes {
    $hashes = [ordered]@{}
    Get-ChildItem -LiteralPath $sourceRoot -File |
        Where-Object { $_.Extension -in '.cpp', '.h', '.ps1' } |
        Sort-Object Name | ForEach-Object {
            $hashes[$_.Name] = (Get-FileHash -LiteralPath $_.FullName -Algorithm SHA256).Hash.ToLowerInvariant()
        }
    return $hashes
}
$sourceBefore = Get-NativeSourceHashes
$common = @('/nologo', '/std:c++20', '/W4', '/WX', '/permissive-', '/EHsc', '/O2', '/Zi', '/MD',
    '/DUNICODE', '/D_UNICODE', '/DNOMINMAX', '/DWIN32_LEAN_AND_MEAN', '/D_WIN32_WINNT=0x0A00',
    "/I$sourceRoot", "/Fo$buildRoot\", "/Fd$buildRoot\compiler.pdb")
$nativeSources = @('observer.cpp', 'native_output.cpp', 'native_gpu.cpp') | ForEach-Object { Join-Path $sourceRoot $_ }
Push-Location $buildRoot
Start-Transcript -LiteralPath (Join-Path $buildRoot 'build.log') | Out-Null
$buildFailure = $null
try {
    & $compiler @common /LD @nativeSources (Join-Path $sourceRoot 'exports.cpp') "/Fe$buildRoot\native_dxgi.dll" /link /INCREMENTAL:NO /DEBUG "/PDB:$buildRoot\native_dxgi.pdb" d3d11.lib dxgi.lib user32.lib
    if ($LASTEXITCODE -ne 0) { throw 'DXGI DLL compile/link failed' }
    & $compiler @common @nativeSources (Join-Path $sourceRoot 'observer_tests.cpp') (Join-Path $sourceRoot 'observer_native_tests.cpp') "/Fe$buildRoot\observer_tests.exe" /link /INCREMENTAL:NO /DEBUG "/PDB:$buildRoot\observer_tests.pdb" d3d11.lib dxgi.lib user32.lib
    if ($LASTEXITCODE -ne 0) { throw 'DXGI owner tests compile/link failed' }
    & $compiler @common (Join-Path $sourceRoot 'color_fixture.cpp') "/Fe$buildRoot\color_fixture.exe" /link /INCREMENTAL:NO /DEBUG "/PDB:$buildRoot\color_fixture.pdb" user32.lib gdi32.lib
    if ($LASTEXITCODE -ne 0) { throw 'Color fixture compile/link failed' }
    & $compiler @common (Join-Path $sourceRoot 'descriptor_probe.cpp') (Join-Path $sourceRoot 'descriptor_update.cpp') (Join-Path $sourceRoot 'desktop_identity.cpp') (Join-Path $sourceRoot 'format_acquisition.cpp') "/Fe$buildRoot\descriptor_probe.exe" /link /INCREMENTAL:NO /DEBUG "/PDB:$buildRoot\descriptor_probe.pdb" d3d11.lib dxgi.lib user32.lib gdi32.lib dwmapi.lib
    if ($LASTEXITCODE -ne 0) { throw 'Noncapture descriptor probe compile/link failed' }
    & $compiler @common (Join-Path $sourceRoot 'descriptor_readiness_tests.cpp') (Join-Path $sourceRoot 'descriptor_update.cpp') (Join-Path $sourceRoot 'desktop_identity.cpp') (Join-Path $sourceRoot 'format_acquisition.cpp') "/Fe$buildRoot\descriptor_readiness_tests.exe" /link /INCREMENTAL:NO /DEBUG "/PDB:$buildRoot\descriptor_readiness_tests.pdb" d3d11.lib dxgi.lib user32.lib gdi32.lib dwmapi.lib
    if ($LASTEXITCODE -ne 0) { throw 'Descriptor readiness tests compile/link failed' }
    if ($RunTests) {
        & (Join-Path $buildRoot 'observer_tests.exe') 2>&1 | Tee-Object -FilePath (Join-Path $buildRoot 'owner-tests.log')
        if ($LASTEXITCODE -ne 0) { throw 'DXGI owner tests failed' }
        & (Join-Path $buildRoot 'descriptor_readiness_tests.exe') 2>&1 | Tee-Object -FilePath (Join-Path $buildRoot 'readiness-tests.log')
        if ($LASTEXITCODE -ne 0) { throw 'Descriptor readiness tests failed' }
    }
} catch { $buildFailure = $_ } finally { Stop-Transcript | Out-Null; Pop-Location }
$sourceAfter = Get-NativeSourceHashes
$sourcesStable = ($sourceBefore | ConvertTo-Json -Compress) -ceq ($sourceAfter | ConvertTo-Json -Compress)
$artifacts = [ordered]@{}
foreach ($name in @('native_dxgi.dll', 'native_dxgi.pdb', 'color_fixture.exe', 'color_fixture.pdb',
        'descriptor_probe.exe', 'descriptor_probe.pdb', 'observer_tests.exe', 'observer_tests.pdb',
        'descriptor_readiness_tests.exe', 'descriptor_readiness_tests.pdb')) {
    $path = Join-Path $buildRoot $name
    if (Test-Path -LiteralPath $path) {
        $artifacts[$name] = (Get-FileHash -LiteralPath $path -Algorithm SHA256).Hash.ToLowerInvariant()
    }
}
$identity = [ordered]@{
    schema_version = 1; vs_instance = $vsRoot; vs_version = $selected[0].catalog.productDisplayVersion
    vc_version = $env:VCToolsVersion; sdk_version = $env:WindowsSDKVersion.TrimEnd('\')
    compiler = $compiler; architecture = 'x64'; flags = $common
    build_root = $buildRoot
    source_hashes_before = $sourceBefore; source_hashes_after = $sourceAfter
    sources_stable = $sourcesStable; artifacts = $artifacts
    accepted_build = $sourcesStable -and $null -eq $buildFailure
    tests_ran = $RunTests.IsPresent
}
$identity | ConvertTo-Json -Depth 5 | Set-Content -LiteralPath (Join-Path $buildRoot 'build-identity.json') -Encoding utf8
$identity | ConvertTo-Json -Depth 5 -Compress
if (-not $sourcesStable) { throw 'Native sources changed during build; artifacts are not accepted' }
if ($null -ne $buildFailure) { throw $buildFailure }
