# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (C) 2026 Artificial Sweetener and contributors
# Exercise native setup maintenance against a private fixture without launching ComfyUI.
param(
    [Parameter(Mandatory)][string]$Setup,
    [Parameter(Mandatory)][string]$Bundle,
    [Parameter(Mandatory)][string]$OutputRoot
)
$ErrorActionPreference = 'Stop'
$Setup = (Resolve-Path -LiteralPath $Setup).Path
$Bundle = (Resolve-Path -LiteralPath $Bundle).Path
$OutputRoot = [IO.Path]::GetFullPath($OutputRoot)
if (Test-Path -LiteralPath $OutputRoot) { throw 'Use a new, dedicated qualification directory.' }
[IO.Directory]::CreateDirectory($OutputRoot) | Out-Null
$installRoot = Join-Path $OutputRoot 'installed'
$original = Join-Path $installRoot 'releases\fixture'
[IO.Directory]::CreateDirectory($original) | Out-Null
$external = Join-Path $OutputRoot 'user-files'
[IO.Directory]::CreateDirectory($external) | Out-Null
$utf8 = [Text.UTF8Encoding]::new($false)
function Write-Json([string]$Path, $Value) {
    [IO.File]::WriteAllText($Path, ($Value | ConvertTo-Json -Depth 10), $utf8)
}
Write-Json (Join-Path $installRoot '.substitute-installation.json') @{
    schema_version = 1; product = 'SugarSubstitute'
}
$runtime = @{
    directory = (Join-Path $external 'ComfyUI'); python = (Join-Path $external 'python.exe')
    frontend = (Join-Path $external 'frontend'); metadata = (Join-Path $external 'METADATA')
}
$selection = @{
    locale = 'ja'; installRoot = $installRoot; mode = 'attached'; comfyRoot = $runtime.directory
    python = ''; models = (Join-Path $external 'models'); outputs = (Join-Path $external 'outputs')
    selectionToken = $null
}
Write-Json (Join-Path $original 'installation-ready.json') @{
    schema_version = 1; bundle_sha256 = ('a' * 64); selection = $selection; runtime = $runtime
}
$config = Join-Path $original 'startup.local.toml'
[IO.File]::WriteAllText($config, "schema_version = 1`r`n# Preserve custom configuration`r`n[backend]`r`nport = 9191`r`n", $utf8)
$launchArguments = @(
    '--startup-config', $config, '--frontend-package', $runtime.frontend,
    '--frontend-metadata', $runtime.metadata, '--sugarcubes-root', (Join-Path $external 'custom cube'),
    '--backend-script', (Join-Path $runtime.directory 'main.py'),
    '--profile-directory', (Join-Path $external 'custom profile')
)
Write-Json (Join-Path $original 'sugarsubstitute-launch.local.json') @{
    schema_version = 1; arguments = $launchArguments
}
[IO.File]::WriteAllText((Join-Path $original 'sugarsubstitute.exe'), 'Unexecuted original fixture', $utf8)
[IO.File]::WriteAllText((Join-Path $external 'authored-workflow.json'), '{"preserve":true}', $utf8)
$shell = New-Object -ComObject WScript.Shell
$link = $shell.CreateShortcut((Join-Path $original 'SugarSubstitute.lnk'))
$link.TargetPath = Join-Path $original 'sugarsubstitute.exe'
$link.WorkingDirectory = $original
$link.Save()
Copy-Item -LiteralPath (Join-Path $original 'SugarSubstitute.lnk') -Destination $installRoot
$savedConfig = (Get-FileHash -LiteralPath $config -Algorithm SHA256).Hash
$savedUser = (Get-FileHash -LiteralPath (Join-Path $external 'authored-workflow.json') -Algorithm SHA256).Hash
$results = @()
foreach ($scenario in @('update', 'missing-entry-repair', 'missing-executable-repair')) {
    $selected = $shell.CreateShortcut((Join-Path $installRoot 'SugarSubstitute.lnk')).TargetPath
    if ($scenario -eq 'missing-entry-repair') {
        Remove-Item -LiteralPath (Join-Path $installRoot 'SugarSubstitute.lnk')
    }
    if ($scenario -eq 'missing-executable-repair') {
        $resolved = [IO.Path]::GetFullPath($selected)
        $ownedRoot = [IO.Path]::GetFullPath((Join-Path $installRoot 'releases')) + '\'
        if (!$resolved.StartsWith($ownedRoot, [StringComparison]::OrdinalIgnoreCase) -or
            [IO.Path]::GetFileName($resolved) -ne 'sugarsubstitute.exe') {
            throw 'Fixture selection escaped its owned generation directory.'
        }
        $readyPath = Join-Path (Split-Path -Parent $resolved) 'installation-ready.json'
        $ready = Get-Content -LiteralPath $readyPath -Raw | ConvertFrom-Json
        $ready.selection.mode = 'managed'
        $ready.selection.selectionToken = 'expired-provisioning-selection'
        Write-Json $readyPath $ready
        Remove-Item -LiteralPath $resolved
    }
    $receipt = Join-Path $OutputRoot "$scenario.json"
    $workspace = Join-Path $OutputRoot "workspace-$scenario"
    $arguments = @('--bundle', $Bundle, '--workspace', $workspace,
        '--qualify-maintenance', $receipt, '--native-installation', $installRoot)
    $start = [Diagnostics.ProcessStartInfo]::new($Setup)
    $start.UseShellExecute = $false
    $start.CreateNoWindow = $true
    foreach ($argument in $arguments) { $start.ArgumentList.Add($argument) }
    $process = [Diagnostics.Process]::Start($start)
    if (!$process.WaitForExit(120000)) {
        $process.Kill($true)
        $process.WaitForExit()
        throw 'Native maintenance failed to settle within the qualification limit.'
    }
    if ($process.ExitCode -ne 0) { throw "Native maintenance failed: $receipt" }
    $result = Get-Content -LiteralPath $receipt -Raw | ConvertFrom-Json
    if (!$result.passed -or !$result.native_maintenance) { throw "Invalid maintenance receipt: $receipt" }
    $replacement = $shell.CreateShortcut((Join-Path $installRoot 'SugarSubstitute.lnk')).TargetPath
    if ($replacement -eq $selected -or $replacement -ne $result.installed_executable) {
        throw 'Maintenance did not publish the reported new generation.'
    }
    $generation = Split-Path -Parent $replacement
    if ((Get-FileHash -LiteralPath $replacement).Hash -ne
        (Get-FileHash -LiteralPath (Join-Path $Bundle 'app\sugarsubstitute.exe')).Hash) {
        throw 'Installed executable differs from the authenticated package.'
    }
    if ((Get-FileHash -LiteralPath (Join-Path $generation 'startup.local.toml')).Hash -ne $savedConfig) {
        throw 'Maintenance changed saved configuration.'
    }
    $descriptor = Get-Content -LiteralPath (Join-Path $generation 'sugarsubstitute-launch.local.json') -Raw | ConvertFrom-Json
    $saved = Get-Content -LiteralPath (Join-Path $generation 'installation-ready.json') -Raw | ConvertFrom-Json
    $expectedMode = if ($scenario -eq 'missing-executable-repair') { 'managed' } else { 'attached' }
    if ($saved.selection.mode -ne $expectedMode) { throw 'Maintenance changed environment ownership.' }
    if ($descriptor.arguments[1] -ne (Join-Path $generation 'startup.local.toml') -or
        (ConvertTo-Json -InputObject @($descriptor.arguments[2..11]) -Compress) -ne
        (ConvertTo-Json -InputObject @($launchArguments[2..11]) -Compress)) {
        throw 'Maintenance changed external launch paths.'
    }
    if ((Get-FileHash -LiteralPath (Join-Path $external 'authored-workflow.json')).Hash -ne $savedUser -or
        (Get-ChildItem -LiteralPath $external).Count -ne 1) {
        throw 'Maintenance changed external files or provisioned an environment.'
    }
    $results += @{ scenario = $scenario; mode = $expectedMode; executable = $replacement; configurationPreserved = $true }
    $process.Dispose()
}
Write-Json (Join-Path $OutputRoot 'maintenance-result.json') @{
    passed = $true; scenarios = $results; provisioningWorkerStarted = $false; userFilesPreserved = $true
}
