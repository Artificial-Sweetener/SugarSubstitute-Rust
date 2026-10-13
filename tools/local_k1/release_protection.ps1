# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (C) 2026 Artificial Sweetener and contributors
# Match the native protected-storage owner, two explicit ACEs and medium NW label.
function New-ReleaseDescriptor {
    $sid=[Security.Principal.WindowsIdentity]::GetCurrent().User.Value
    $sddl="O:${sid}D:P(A;OICI;FA;;;${sid})(A;OICI;FA;;;SY)S:(ML;;NW;;;ME)"
    $descriptor=[Security.AccessControl.RawSecurityDescriptor]::new($sddl)
    $bytes=[byte[]]::new($descriptor.BinaryLength)
    $descriptor.GetBinaryForm($bytes,0)
    return ,$bytes
}

# Source and destination remain the same actual pinned files through verification.
function Copy-ReleaseFile($Source,[string]$Destination,[byte[]]$Descriptor) {
    if ($Source.Stream.Length -le 0 -or $Source.Stream.Length -gt 1073741824) { throw 'Release source size bound.' }
    $target=$null;$cause=$null;$facts=$null
    try {
        $target=[LocalK1PrivateFiles]::CreateProtectedNew($Destination,$Descriptor)
        $Source.Stream.Position=0
        $buffer=[byte[]]::new(65536)
        $remaining=$Source.Stream.Length
        while ($remaining -gt 0) {
            $count=[int][Math]::Min($remaining,$buffer.Length)
            $Source.Stream.ReadExactly($buffer,0,$count)
            $target.Stream.Write($buffer,0,$count)
            $remaining-=$count
        }
        $target.Stream.Flush($true)
        $Source.Stream.Position=0;$target.Stream.Position=0
        $sourceHash=[Security.Cryptography.SHA256]::HashData($Source.Stream)
        $targetHash=[Security.Cryptography.SHA256]::HashData($target.Stream)
        if ($Source.Stream.Length -ne $target.Stream.Length -or
            [Convert]::ToHexString($sourceHash) -cne [Convert]::ToHexString($targetHash)) {
            throw 'Same-file protected copy verification failed.'
        }
        $facts=[ordered]@{file=[IO.Path]::GetFileName($Destination);bytes=$target.Stream.Length;
            sha256=[Convert]::ToHexString($targetHash).ToLowerInvariant()}
    } catch { $cause=$_.Exception }
    finally { $cause=Invoke-ToolCleanup $cause @({ if ($null -ne $target) { [LocalK1PrivateFiles]::Close($target) } }) }
    if ($null -ne $cause) { throw $cause }
    return $facts
}
