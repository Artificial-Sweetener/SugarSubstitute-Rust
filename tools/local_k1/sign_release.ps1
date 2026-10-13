# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (C) 2026 Artificial Sweetener and contributors
# Sign an actual final local-K1 host/capture bundle using the existing isolated signer.
[CmdletBinding()]
param(
    [Parameter(Mandatory)][string]$PrivateRoot,
    [Parameter(Mandatory)][string]$ReleaseRoot,
    [Parameter(Mandatory)][string]$TemporaryRoot,
    [Parameter(Mandatory)][string]$PublicAnchor,
    [Parameter(Mandatory)][string]$BuildLiteral,
    [Parameter(Mandatory)][string]$HostImage,
    [Parameter(Mandatory)][string]$Python
)
Set-StrictMode -Version Latest
$ErrorActionPreference='Stop'
. (Join-Path $PSScriptRoot 'paths.ps1')
. (Join-Path $PSScriptRoot 'signing.ps1')
. (Join-Path $PSScriptRoot 'release_protection.ps1')
$privateRoot=Resolve-TaskRoot $PrivateRoot
$releaseRoot=Resolve-TaskRoot $ReleaseRoot
$temporaryRoot=Resolve-TaskRoot $TemporaryRoot
Assert-CustodyRoots $privateRoot $releaseRoot $temporaryRoot
foreach ($path in @($PublicAnchor,$BuildLiteral,$Python)) {
    if (-not [IO.Path]::IsPathFullyQualified($path) -or $path.StartsWith('\\')) {
        throw 'Inputs must be explicit absolute local paths.'
    }
}
if ($HostImage -cnotmatch '^[a-z0-9_-]+\.exe$') { throw 'Host image must be one canonical executable leaf.' }
$hostPdb=$HostImage.Substring(0,$HostImage.Length-4)+'.pdb'
$names=@($HostImage,$hostPdb,'sugarsubstitute_crashpad_client.dll','sugarsubstitute_crashpad_client.pdb',
    'crashpad_handler.exe','crashpad_handler.pdb')
if (@($names | Select-Object -Unique).Count -ne 6) { throw 'Release roles must have six distinct files.' }
$manifestPath=Join-Path $releaseRoot 'sugarsubstitute-release-v1.manifest'
$signaturePath=Join-Path $releaseRoot 'sugarsubstitute-release-v1.sig'
$proofPath=Join-Path $releaseRoot 'local-k1-signature-receipt.json'
foreach ($path in @($manifestPath,$signaturePath,$proofPath,(Join-Path $releaseRoot 'local-k1-manifest-inputs.json'))) {
    if (Test-Path -LiteralPath $path) { throw 'Final signing outputs already exist; preserve that exact attempt.' }
}
$oldTemp=$env:TEMP;$oldTmp=$env:TMP
$oldCache=$env:PYTHONPYCACHEPREFIX
$oldNoBytecode=$env:PYTHONDONTWRITEBYTECODE
$env:TEMP=$temporaryRoot;$env:TMP=$temporaryRoot
$env:PYTHONPYCACHEPREFIX=Join-Path $temporaryRoot 'pythoncache'
$env:PYTHONDONTWRITEBYTECODE='1'
$pins=$null;$cause=$null;$ready=$false
try {
    Add-Type -LiteralPath (Join-Path $PSScriptRoot 'private_files.cs')
    $ready=$true
    $pins=[Collections.Generic.List[LocalK1PrivateFiles+Lease]]::new()
    Add-AncestryPins $releaseRoot $pins
    Add-AncestryPins $temporaryRoot $pins
    Add-AncestryPins ([IO.Directory]::GetParent([IO.Path]::GetFullPath($PublicAnchor)).FullName) $pins
    Add-AncestryPins ([IO.Directory]::GetParent([IO.Path]::GetFullPath($BuildLiteral)).FullName) $pins
    foreach ($name in $names) { $pins.Add([LocalK1PrivateFiles]::OpenRead((Join-Path $releaseRoot $name))) }
    $anchor=[LocalK1PrivateFiles]::OpenRead([IO.Path]::GetFullPath($PublicAnchor));$pins.Add($anchor)
    $literal=[LocalK1PrivateFiles]::OpenRead([IO.Path]::GetFullPath($BuildLiteral));$pins.Add($literal)
    if ($anchor.Stream.Length -ne 411) { throw 'Public anchor bound.' }
    $blob=[byte[]]::new(411);$anchor.Stream.ReadExactly($blob)
    # All six original files and ancestors exclude replacement/writes while the
    # sole bounded producer parses and hashes the actual same open descriptors.
    & $Python (Join-Path $PSScriptRoot 'build_manifest.py') --release-root $releaseRoot --host-image $HostImage `
        --build-literal $BuildLiteral --public-anchor $PublicAnchor --output-root $temporaryRoot
    if ($LASTEXITCODE -ne 0) { throw 'Actual PE/PDB manifest producer failed before private signing.' }
    $descriptor=New-ReleaseDescriptor
    foreach ($name in @('sugarsubstitute-release-v1.manifest','local-k1-manifest-inputs.json')) {
        $produced=[LocalK1PrivateFiles]::OpenRead((Join-Path $temporaryRoot $name));$pins.Add($produced)
        $null=Copy-ReleaseFile $produced (Join-Path $releaseRoot $name) $descriptor
    }
    $manifest=[LocalK1PrivateFiles]::OpenRead($manifestPath);$pins.Add($manifest)
    if ($manifest.Stream.Length -le 0 -or $manifest.Stream.Length -gt 65536) { throw 'Manifest bound.' }
    $message=[byte[]]::new([int]$manifest.Stream.Length);$manifest.Stream.ReadExactly($message)
    $signed=New-LocalK1Signature $privateRoot $blob $message
    [LocalK1PrivateFiles]::WriteProtectedNew($signaturePath,$signed.signature,$descriptor)
    $proof=[ordered]@{schema=1;identity='isolated-local-k1-development-only';key_id=$signed.key_id;
        manifest_sha256=[Convert]::ToHexString([Security.Cryptography.SHA256]::HashData($message)).ToLowerInvariant();
        signature_sha256=[Convert]::ToHexString([Security.Cryptography.SHA256]::HashData($signed.signature)).ToLowerInvariant();
        signature_bytes=$signed.signature.Length;rsa_provider=$signed.provider;actual_cng_pss_sha256_verified=$true;
        actual_file_roles=@(3,8,16);production_release_identity=$false;private_bytes_exported=$false}
    [LocalK1PrivateFiles]::WriteProtectedNew($proofPath,[Text.Encoding]::UTF8.GetBytes(($proof|ConvertTo-Json -Depth 4)+"`n"),$descriptor)
} catch { $cause=$_.Exception }
finally {
    try {
        $actions=[Collections.Generic.List[scriptblock]]::new()
        if ($ready -and $null -ne $pins) {
            for ($index=$pins.Count-1;$index -ge 0;$index--) {
                $pin=$pins[$index]
                $actions.Add({ [LocalK1PrivateFiles]::Close($pin) }.GetNewClosure())
            }
        }
        $cause=Invoke-ToolCleanup $cause $actions.ToArray()
    } finally {
        $env:TEMP=$oldTemp;$env:TMP=$oldTmp
        $env:PYTHONPYCACHEPREFIX=$oldCache;$env:PYTHONDONTWRITEBYTECODE=$oldNoBytecode
    }
}
if ($null -ne $cause) { throw $cause }
[pscustomobject]@{manifest=$manifestPath;signature=$signaturePath;receipt=$proofPath;identity='isolated-local-k1-development-only'}
