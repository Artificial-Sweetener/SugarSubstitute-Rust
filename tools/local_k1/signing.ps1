# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (C) 2026 Artificial Sweetener and contributors
# Own the existing isolated development key for one bounded signature operation.
function New-LocalK1Signature([string]$PrivateRoot,[byte[]]$PublicBlob,[byte[]]$Message) {
    $expectedKey='790bf1193d5cfd7efa58436532b10c09162d5efd5200b40a9d86fc71257753a5'
    if ($PublicBlob.Length -ne 411 -or $Message.Length -le 0 -or $Message.Length -gt 65536 -or
        [Convert]::ToHexString([Security.Cryptography.SHA256]::HashData($PublicBlob)).ToLowerInvariant() -ne $expectedKey) {
        throw 'Message or reviewed development public identity rejected.'
    }
    $privateRoot=Resolve-TaskRoot $PrivateRoot
    $pins=[Collections.Generic.List[LocalK1PrivateFiles+Lease]]::new()
    $keyFile=$null
    $protectedBytes=$null
    $privateBytes=$null
    $rsa=$null
    $cause=$null
    $signature=$null
    $provider=$null
    try {
        Add-AncestryPins $privateRoot $pins
        $sid=[Security.Principal.WindowsIdentity]::GetCurrent().User
        $keyPath=Join-Path $privateRoot 'signer.pkcs8.dpapi'
        foreach ($path in @([IO.Directory]::GetParent($privateRoot).FullName,$privateRoot,$keyPath)) {
            $acl=Get-Acl -LiteralPath $path
            $rules=@($acl.GetAccessRules($true,$true,[Security.Principal.SecurityIdentifier]))
            if ($acl.GetOwner([Security.Principal.SecurityIdentifier]) -ne $sid -or $rules.Count -ne 1 -or
                $rules[0].IdentityReference -ne $sid -or $rules[0].AccessControlType -ne [Security.AccessControl.AccessControlType]::Allow -or
                $rules[0].FileSystemRights -ne [Security.AccessControl.FileSystemRights]::FullControl) {
                throw 'Stored signer is not current-user-only.'
            }
            if ([IO.Directory]::Exists($path) -and -not $acl.AreAccessRulesProtected) {
                throw 'Private directory inheritance is not protected.'
            }
        }
        $keyFile=[LocalK1PrivateFiles]::OpenRead($keyPath)
        [LocalK1PrivateFiles]::Validate($keyFile,$keyPath,$false)
        if ($keyFile.Stream.Length -le 0 -or $keyFile.Stream.Length -gt 16384) {
            throw 'Stored signer ciphertext bound.'
        }
        $protectedBytes=[byte[]]::new([int]$keyFile.Stream.Length)
        $keyFile.Stream.ReadExactly($protectedBytes)
        $entropy=[Text.Encoding]::UTF8.GetBytes('SugarSubstitute.LocalK1.SymbolSigner.v1')
        $privateBytes=[Security.Cryptography.ProtectedData]::Unprotect($protectedBytes,$entropy,[Security.Cryptography.DataProtectionScope]::CurrentUser)
        if ($privateBytes.Length -gt 8192) { throw 'Stored signer plaintext bound.' }
        $rsa=[LocalK1PrivateFiles]::ImportPrivate($privateBytes)
        [LocalK1PrivateFiles]::Clear($privateBytes)
        $privateBytes=$null
        $parameters=$rsa.ExportParameters($false)
        if ($parameters.Exponent.Length -ne 3 -or $parameters.Modulus.Length -ne 384) {
            throw 'Stored signer public shape rejected.'
        }
        $blob=[byte[]]::new(411)
        $header=[Convert]::FromHexString('52534131000C000003000000800100000000000000000000')
        [Array]::Copy($header,0,$blob,0,24)
        [Array]::Copy($parameters.Exponent,0,$blob,24,3)
        [Array]::Copy($parameters.Modulus,0,$blob,27,384)
        if ([Convert]::ToHexString($blob) -cne [Convert]::ToHexString($PublicBlob)) {
            throw 'Stored signer differs from the reviewed public anchor.'
        }
        $provider=$rsa.GetType().FullName
        $signature=$rsa.SignData($Message,[Security.Cryptography.HashAlgorithmName]::SHA256,[Security.Cryptography.RSASignaturePadding]::Pss)
        if ($signature.Length -ne 384 -or -not $rsa.VerifyData($Message,$signature,
            [Security.Cryptography.HashAlgorithmName]::SHA256,[Security.Cryptography.RSASignaturePadding]::Pss)) {
            throw 'Stored signer signature self-verification failed.'
        }
        # This public verifier uses the same System32 CNG PSS/SHA256/salt32 boundary.
        if (-not [LocalK1PublicVerification]::Verify($PublicBlob,$Message,$signature)) {
            throw 'Actual CNG development signature verification failed.'
        }
    } catch { $cause=$_.Exception }
    finally {
        $actions=[Collections.Generic.List[scriptblock]]::new()
        $actions.Add({ [LocalK1PrivateFiles]::Clear($privateBytes) })
        $actions.Add({ [LocalK1PrivateFiles]::Clear($protectedBytes) })
        $actions.Add({ if ($null -ne $rsa) { $rsa.Dispose() } })
        $actions.Add({ if ($null -ne $keyFile) { [LocalK1PrivateFiles]::Close($keyFile) } })
        for ($index=$pins.Count-1;$index -ge 0;$index--) {
            $pin=$pins[$index]
            $actions.Add({ [LocalK1PrivateFiles]::Close($pin) }.GetNewClosure())
        }
        $cause=Invoke-ToolCleanup $cause $actions.ToArray()
    }
    if ($null -ne $cause) { throw $cause }
    return [pscustomobject]@{signature=$signature;key_id=$expectedKey;provider=$provider}
}
