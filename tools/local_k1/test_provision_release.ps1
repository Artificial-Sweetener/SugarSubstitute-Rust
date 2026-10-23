# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (C) 2026 Artificial Sweetener and contributors
# Exercise only public isolated files, never provisioning or using a signer.
[CmdletBinding()]
param([Parameter(Mandatory)][string]$FixtureRoot)
Set-StrictMode -Version Latest
$ErrorActionPreference='Stop'
. (Join-Path $PSScriptRoot 'paths.ps1')
. (Join-Path $PSScriptRoot 'release_protection.ps1')
$oldTemp=$env:TEMP;$oldTmp=$env:TMP
try {
$root=Resolve-TaskRoot $FixtureRoot
if (Test-Path -LiteralPath $root) { throw 'Fixture root must be fresh.' }
[IO.Directory]::CreateDirectory($root) | Out-Null
$temporary=Join-Path $root 'temp';[IO.Directory]::CreateDirectory($temporary) | Out-Null
$sources=@()
for ($index=0;$index -lt 6;$index++) {
    $source=Join-Path $root "source$index.bin"
    [IO.File]::WriteAllBytes($source,[Text.Encoding]::UTF8.GetBytes("actual public input $index"))
    $sources+=,$source
}
$release=Join-Path $root 'installed'
$result=& (Join-Path $PSScriptRoot 'provision_release.ps1') -ReleaseRoot $release -TemporaryRoot $temporary `
    -HostImage 'fixture.exe' -HostExecutable $sources[0] -HostPdb $sources[1] -ClientDll $sources[2] `
    -ClientPdb $sources[3] -HandlerExecutable $sources[4] -HandlerPdb $sources[5]
if ($result.files -ne 6) { throw 'Actual copy count.' }
$contract = Join-Path $root 'sugarsubstitute-application.json'
$contractBytes = [Text.Encoding]::UTF8.GetBytes('{"schema_version":1,"installed_launch_protocols":[1]}')
[IO.File]::WriteAllBytes($contract, $contractBytes)
$declaredRelease = Join-Path $root 'declared'
$declared = & (Join-Path $PSScriptRoot 'provision_release.ps1') -ReleaseRoot $declaredRelease -TemporaryRoot $temporary `
    -HostImage 'fixture.exe' -HostExecutable $sources[0] -HostPdb $sources[1] -ClientDll $sources[2] `
    -ClientPdb $sources[3] -HandlerExecutable $sources[4] -HandlerPdb $sources[5] -ApplicationContract $contract
if ($declared.files -ne 7 -or [Convert]::ToHexString([IO.File]::ReadAllBytes((Join-Path $declaredRelease 'sugarsubstitute-application.json'))) -cne [Convert]::ToHexString($contractBytes)) {
    throw 'Producing application declaration was not preserved exactly.'
}
$oversized = Join-Path $root 'oversized.json'
[IO.File]::WriteAllBytes($oversized, [byte[]]::new(4097))
$rejectedRoot = Join-Path $root 'rejected'
$oversizedRejected = $false
try {
    & (Join-Path $PSScriptRoot 'provision_release.ps1') -ReleaseRoot $rejectedRoot -TemporaryRoot $temporary `
        -HostImage 'fixture.exe' -HostExecutable $sources[0] -HostPdb $sources[1] -ClientDll $sources[2] `
        -ClientPdb $sources[3] -HandlerExecutable $sources[4] -HandlerPdb $sources[5] -ApplicationContract $oversized | Out-Null
} catch { $oversizedRejected = $_.Exception.Message -like '*contract exceeds*' }
if (-not $oversizedRejected -or (Test-Path -LiteralPath $rejectedRoot)) { throw 'Oversized declaration reached publication.' }
$pairedRelease = Join-Path $root 'paired'
$launcherContract = Join-Path $root 'sugarsubstitute-launcher.json'
$launcherBytes = [Text.Encoding]::UTF8.GetBytes('{"schema_version":1,"installation_schemas":[1],"installed_launch_protocol":1}')
[IO.File]::WriteAllBytes($launcherContract, $launcherBytes)
$updaterContract = Join-Path $root 'sugarsubstitute-updater.json'
[IO.File]::WriteAllBytes($updaterContract, [Text.Encoding]::UTF8.GetBytes('{"schema_version":1,"preparation_protocols":[1],"installation_schemas":[1]}'))
$pairedArguments = @{
    ReleaseRoot=$pairedRelease; TemporaryRoot=$temporary; HostImage='fixture.exe';
    HostExecutable=$sources[0]; HostPdb=$sources[1]; ClientDll=$sources[2]; ClientPdb=$sources[3];
    HandlerExecutable=$sources[4]; HandlerPdb=$sources[5]; ApplicationContract=$contract;
    LauncherExecutable=$sources[0]; LauncherPdb=$sources[1]; LauncherContract=$launcherContract;
    UpdaterExecutable=$sources[0]; UpdaterPdb=$sources[1]; UpdaterContract=$updaterContract
}
$paired = & (Join-Path $PSScriptRoot 'provision_release.ps1') @pairedArguments
if ($paired.files -ne 13) { throw 'Paired release copy count.' }
foreach ($item in @(
    @('sugarsubstitute-launcher.exe', $sources[0]),
    @('sugarsubstitute_launcher.pdb', $sources[1]),
    @('sugarsubstitute-launcher.json', $launcherContract),
    @('sugarsubstitute-updater.exe', $sources[0]),
    @('sugarsubstitute_updater.pdb', $sources[1]),
    @('sugarsubstitute-updater.json', $updaterContract)
)) {
    if ([Convert]::ToHexString([IO.File]::ReadAllBytes((Join-Path $pairedRelease $item[0]))) -cne
        [Convert]::ToHexString([IO.File]::ReadAllBytes($item[1]))) { throw 'Launcher payload copy changed.' }
}
foreach ($missing in @('LauncherExecutable','LauncherPdb','LauncherContract','ApplicationContract','UpdaterExecutable','UpdaterPdb','UpdaterContract')) {
    $incomplete = $pairedArguments.Clone()
    $incomplete.Remove($missing)
    $incomplete.ReleaseRoot = Join-Path $root "missing-$missing"
    $failed = $false
    try { & (Join-Path $PSScriptRoot 'provision_release.ps1') @incomplete | Out-Null }
    catch { $failed = $_.Exception.Message -match '(Launcher|Updater) requires' }
    if (-not $failed -or (Test-Path -LiteralPath $incomplete.ReleaseRoot)) { throw 'Incomplete executable role reached publication.' }
}
foreach ($name in @('ApplicationContract', 'LauncherContract', 'UpdaterContract')) {
    $invalid = $pairedArguments.Clone()
    $invalid[$name] = $oversized
    $invalid.ReleaseRoot = Join-Path $root "oversized-$name"
    $failed = $false
    try { & (Join-Path $PSScriptRoot 'provision_release.ps1') @invalid | Out-Null }
    catch { $failed = $_.Exception.Message -like '*contract exceeds*' }
    if (-not $failed -or (Test-Path -LiteralPath $invalid.ReleaseRoot)) { throw 'Oversized paired declaration reached publication.' }
}
$env:TEMP=$temporary;$env:TMP=$temporary
Add-Type -TypeDefinition @'
using System;
using System.Collections.Generic;
using System.ComponentModel;
using System.Runtime.InteropServices;
using Microsoft.Win32.SafeHandles;
public static class ReleaseSecurityObservation {
    static readonly List<IntPtr> retained=new List<IntPtr>();
    [DllImport("advapi32.dll")] static extern uint GetSecurityInfo(SafeFileHandle handle,uint kind,uint info,
        out IntPtr owner,out IntPtr group,out IntPtr dacl,out IntPtr sacl,out IntPtr descriptor);
    [DllImport("advapi32.dll")] static extern uint GetSecurityDescriptorLength(IntPtr descriptor);
    [DllImport("kernel32.dll")] static extern IntPtr LocalFree(IntPtr pointer);
    public static byte[] Read(SafeFileHandle handle) {
        IntPtr owner,group,dacl,sacl,descriptor;
        uint status=GetSecurityInfo(handle,1,0x15,out owner,out group,out dacl,out sacl,out descriptor);
        if(descriptor!=IntPtr.Zero) retained.Add(descriptor);
        if(status!=0) throw new Win32Exception((int)status);
        uint count=GetSecurityDescriptorLength(descriptor);
        if(count<20||count>8192) throw new InvalidOperationException("Descriptor bound");
        byte[] bytes=new byte[count];Marshal.Copy(descriptor,bytes,0,bytes.Length);
        if(LocalFree(descriptor)!=IntPtr.Zero) throw new InvalidOperationException("Descriptor retirement uncertain");
        retained.Remove(descriptor);return bytes;
    }
}
'@
$expected=[Security.AccessControl.RawSecurityDescriptor]::new((New-ReleaseDescriptor),0)
$labels=@('fixture.exe','fixture.pdb','sugarsubstitute_crashpad_client.dll',
    'sugarsubstitute_crashpad_client.pdb','crashpad_handler.exe','crashpad_handler.pdb')
$checked=0
foreach ($path in @($release)+@($labels | ForEach-Object { Join-Path $release $_ })+@($declaredRelease,(Join-Path $declaredRelease 'sugarsubstitute-application.json'))+@($pairedRelease)+@('sugarsubstitute-launcher.exe','sugarsubstitute_launcher.pdb','sugarsubstitute-launcher.json','sugarsubstitute-updater.exe','sugarsubstitute_updater.pdb','sugarsubstitute-updater.json' | ForEach-Object { Join-Path $pairedRelease $_ })) {
    $pin=$null;$cause=$null
    try {
        if ($path -in @($release,$declaredRelease,$pairedRelease)) { $pin=[LocalK1PrivateFiles]::Pin($path);$handle=$null }
        else { $pin=[LocalK1PrivateFiles]::OpenRead($path);$handle=$pin.Stream.SafeFileHandle }
        # Lease.borrowed is internal; obtain the directory's same observed raw owner
        # through the test-only reflection boundary, never reopen its path.
        if ($path -in @($release,$declaredRelease,$pairedRelease)) {
            $handle=$pin.GetType().GetField('borrowed',[Reflection.BindingFlags]'NonPublic,Instance').GetValue($pin)
        }
        $actual=[Security.AccessControl.RawSecurityDescriptor]::new([ReleaseSecurityObservation]::Read($handle),0)
        if ($actual.Owner -ne $expected.Owner -or ($actual.ControlFlags -band 4096) -ne 4096) { throw 'Actual owner/protected DACL mismatch.' }
        foreach ($pair in @(@($actual.DiscretionaryAcl,$expected.DiscretionaryAcl),@($actual.SystemAcl,$expected.SystemAcl))) {
            $left=[byte[]]::new($pair[0].BinaryLength);$right=[byte[]]::new($pair[1].BinaryLength)
            $pair[0].GetBinaryForm($left,0);$pair[1].GetBinaryForm($right,0)
            if ([Convert]::ToHexString($left) -cne [Convert]::ToHexString($right)) { throw 'Actual exact ACE/medium label mismatch.' }
        }
        $checked++
    } catch { $cause=$_.Exception }
    finally { $cause=Invoke-ToolCleanup $cause @({ if ($null -ne $pin) { [LocalK1PrivateFiles]::Close($pin) } }) }
    if ($null -ne $cause) { throw $cause }
}
for ($index=0;$index -lt 6;$index++) {
    if ([Convert]::ToHexString([IO.File]::ReadAllBytes($sources[$index])) -cne
        [Convert]::ToHexString([IO.File]::ReadAllBytes((Join-Path $release $labels[$index])))) { throw 'Copied bytes changed.' }
}
$rejected=$false
try { & (Join-Path $PSScriptRoot 'provision_release.ps1') -ReleaseRoot $release -TemporaryRoot $temporary `
    -HostImage 'fixture.exe' -HostExecutable $sources[0] -HostPdb $sources[1] -ClientDll $sources[2] `
    -ClientPdb $sources[3] -HandlerExecutable $sources[4] -HandlerPdb $sources[5] | Out-Null }
catch { $rejected=$_.Exception.Message -like '*already exists*' }
if (-not $rejected) { throw 'Existing root was not rejected.' }
[pscustomobject]@{protected_native_objects=$checked;exact_copies=6;application_contract_exact_copy=$true;launcher_exact_copies=3;updater_exact_copies=3;incomplete_updater_rejected=$true;incomplete_launcher_rejected=$true;oversized_contract_rejected=$true;existing_root_rejected=$true;private_key_effects=0}
} finally { $env:TEMP=$oldTemp;$env:TMP=$oldTmp }
