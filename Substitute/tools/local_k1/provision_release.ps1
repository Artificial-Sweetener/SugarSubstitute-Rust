# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (C) 2026 Artificial Sweetener and contributors
# Create one fresh protected bundle from explicit runtime files and an optional build contract.
[CmdletBinding()]
param(
    [Parameter(Mandatory)][string]$ReleaseRoot,
    [Parameter(Mandatory)][string]$TemporaryRoot,
    [Parameter(Mandatory)][string]$HostImage,
    [Parameter(Mandatory)][string]$HostExecutable,
    [Parameter(Mandatory)][string]$HostPdb,
    [Parameter(Mandatory)][string]$ClientDll,
    [Parameter(Mandatory)][string]$ClientPdb,
    [Parameter(Mandatory)][string]$HandlerExecutable,
    [Parameter(Mandatory)][string]$HandlerPdb,
    [string]$ApplicationContract,
    [string]$LauncherExecutable,
    [string]$LauncherPdb,
    [string]$LauncherContract,
    [string]$InstallationActivationContract,
    [string]$UpdaterExecutable,
    [string]$UpdaterPdb,
    [string]$UpdaterContract
)
Set-StrictMode -Version Latest
$ErrorActionPreference='Stop'
. (Join-Path $PSScriptRoot 'paths.ps1')
. (Join-Path $PSScriptRoot 'release_protection.ps1')
$releaseRoot=Resolve-TaskRoot $ReleaseRoot
$temporaryRoot=Resolve-TaskRoot $TemporaryRoot
if (Test-Path -LiteralPath $releaseRoot) { throw 'Release root already exists; preserve its entire original contents.' }
if (-not [IO.Directory]::Exists($temporaryRoot)) { throw 'Explicit temporary root must already exist.' }
if ($temporaryRoot.StartsWith($releaseRoot+'\',[StringComparison]::OrdinalIgnoreCase)) { throw 'Fresh release root cannot contain preexisting temporary files.' }
$parent=[IO.Directory]::GetParent($releaseRoot).FullName
if (-not [IO.Directory]::Exists($parent)) { throw 'Release parent must already exist.' }
if ($HostImage -cnotmatch '^[a-z0-9_-]+\.exe$') { throw 'Host image must be one canonical executable leaf.' }
$labels=@($HostImage,($HostImage.Substring(0,$HostImage.Length-4)+'.pdb'),
    'sugarsubstitute_crashpad_client.dll','sugarsubstitute_crashpad_client.pdb','crashpad_handler.exe','crashpad_handler.pdb')
if (@($labels | Select-Object -Unique).Count -ne 6) { throw 'Release roles must be distinct.' }
$sources=@($HostExecutable,$HostPdb,$ClientDll,$ClientPdb,$HandlerExecutable,$HandlerPdb)
if ($ApplicationContract) {
    $labels += 'sugarsubstitute-application.json'
    $sources += $ApplicationContract
}
$launcherInputs = @($LauncherExecutable, $LauncherPdb, $LauncherContract, $InstallationActivationContract | Where-Object { $_ })
if ($launcherInputs.Count -gt 0) {
    if ($launcherInputs.Count -ne 4 -or -not $ApplicationContract) { throw 'Launcher requires its executable, symbols and producing application, launcher and installation activation contracts.' }
    $labels += @('sugarsubstitute-launcher.exe', 'sugarsubstitute_launcher.pdb', 'sugarsubstitute-launcher.json', 'sugarsubstitute-installation-activation.json')
    $sources += @($LauncherExecutable, $LauncherPdb, $LauncherContract, $InstallationActivationContract)
}
$updaterInputs = @($UpdaterExecutable, $UpdaterPdb, $UpdaterContract | Where-Object { $_ })
if ($updaterInputs.Count -gt 0) {
    if ($updaterInputs.Count -ne 3) { throw 'Updater requires its executable, symbols and producing entry contract.' }
    $labels += @('sugarsubstitute-updater.exe', 'sugarsubstitute_updater.pdb', 'sugarsubstitute-updater.json')
    $sources += @($UpdaterExecutable, $UpdaterPdb, $UpdaterContract)
}
if (@($labels | Select-Object -Unique).Count -ne $labels.Count) { throw 'Release roles must be distinct.' }
foreach ($source in $sources) {
    if (-not [IO.Path]::IsPathFullyQualified($source) -or $source.StartsWith('\\')) { throw 'Source files must be explicit absolute local paths.' }
}
$oldTemp=$env:TEMP;$oldTmp=$env:TMP
$env:TEMP=$temporaryRoot;$env:TMP=$temporaryRoot
$pins=$null;$cause=$null
try {
    Add-Type -LiteralPath (Join-Path $PSScriptRoot 'private_files.cs')
    $pins=[Collections.Generic.List[LocalK1PrivateFiles+Lease]]::new()
    Add-AncestryPins $parent $pins
    Add-AncestryPins $temporaryRoot $pins
    $opened=[Collections.Generic.List[LocalK1PrivateFiles+Lease]]::new()
    foreach ($source in $sources) {
        $path=[IO.Path]::GetFullPath($source)
        Add-AncestryPins ([IO.Directory]::GetParent($path).FullName) $pins
        $file=[LocalK1PrivateFiles]::OpenRead($path);$pins.Add($file);$opened.Add($file)
    }
    for ($index=0;$index -lt $labels.Count;$index++) {
        if ($labels[$index] -in @('sugarsubstitute-application.json','sugarsubstitute-launcher.json','sugarsubstitute-updater.json','sugarsubstitute-installation-activation.json') -and
            ($opened[$index].Stream.Length -le 0 -or $opened[$index].Stream.Length -gt 4096)) {
            throw 'Launch contract exceeds its native bound.'
        }
    }
    $descriptor=New-ReleaseDescriptor
    [LocalK1PrivateFiles]::CreateProtectedDirectoryNew($releaseRoot,$descriptor)
    $pins.Add([LocalK1PrivateFiles]::Pin($releaseRoot))
    $facts=[Collections.Generic.List[object]]::new()
    for ($index=0;$index -lt $labels.Count;$index++) {
        $facts.Add((Copy-ReleaseFile $opened[$index] (Join-Path $releaseRoot $labels[$index]) $descriptor))
    }
    $receipt=[ordered]@{schema=1;owner=[Security.Principal.WindowsIdentity]::GetCurrent().User.Value;
        dacl='protected-current-user-and-system-full-control-object-container-inherit';
        integrity='medium-no-write-up';files=$facts.ToArray();scope='protected copies; release authentication requires detached signature'}
    [LocalK1PrivateFiles]::WriteProtectedNew((Join-Path $releaseRoot 'local-k1-protected-copy.json'),
        [Text.Encoding]::UTF8.GetBytes(($receipt | ConvertTo-Json -Depth 5)+"`n"),$descriptor)
} catch { $cause=$_.Exception }
finally {
    try {
        $actions=[Collections.Generic.List[scriptblock]]::new()
        if ($null -ne $pins) {
            for ($index=$pins.Count-1;$index -ge 0;$index--) {
                $pin=$pins[$index];$actions.Add({ [LocalK1PrivateFiles]::Close($pin) }.GetNewClosure())
            }
        }
        $cause=Invoke-ToolCleanup $cause $actions.ToArray()
    } finally { $env:TEMP=$oldTemp;$env:TMP=$oldTmp }
}
if ($null -ne $cause) { throw $cause }
[pscustomobject]@{release_root=$releaseRoot;files=$labels.Count;signature_required=$true}
