# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (C) 2026 Artificial Sweetener and contributors
# Resolve explicit local task roots before temporary compilation or private effects.
function Resolve-TaskRoot([string]$Path) {
    if (-not [IO.Path]::IsPathFullyQualified($Path) -or $Path.StartsWith('\\')) {
        throw 'Task roots must be explicit absolute local paths.'
    }
    $resolved=[IO.Path]::GetFullPath($Path).TrimEnd('\')
    if ($resolved.Length -le 3) { throw 'Task roots must be below the drive root.' }
    $cursor=$resolved
    while ($cursor) {
        if ([IO.File]::Exists($cursor)) { throw 'Task directory ancestry contains a file.' }
        if ([IO.Directory]::Exists($cursor)) {
            $attributes=[IO.File]::GetAttributes($cursor)
            if (($attributes -band [IO.FileAttributes]::ReparsePoint) -ne 0) { throw 'Task ancestry contains a reparse point.' }
        }
        $parent=[IO.Directory]::GetParent($cursor)
        if ($null -eq $parent) { break }
        $cursor=$parent.FullName
    }
    return $resolved
}
function Assert-CustodyRoots([string]$PrivateRoot,[string]$PublicRoot,[string]$TemporaryRoot) {
    if (-not $TemporaryRoot.StartsWith($PublicRoot+'\',[StringComparison]::OrdinalIgnoreCase)) {
        throw 'Temporary compilation must remain within the explicit public output root.'
    }
    if ($PrivateRoot -and ($PrivateRoot.Equals($PublicRoot,[StringComparison]::OrdinalIgnoreCase) -or
        $PrivateRoot.StartsWith($PublicRoot+'\',[StringComparison]::OrdinalIgnoreCase) -or
        $PublicRoot.StartsWith($PrivateRoot+'\',[StringComparison]::OrdinalIgnoreCase))) {
        throw 'Private signer and public output roots must have disjoint custody.'
    }
}
function Add-AncestryPins([string]$Path,$Pins) {
    $ancestry=[Collections.Generic.List[string]]::new()
    $cursor=[IO.DirectoryInfo]::new($Path)
    while ($null -ne $cursor) { $ancestry.Add($cursor.FullName);$cursor=$cursor.Parent }
    for ($index=$ancestry.Count-1;$index -ge 0;$index--) {
        $Pins.Add([LocalK1PrivateFiles]::Pin($ancestry[$index]))
    }
}
function Assert-ProofLeaf([string]$Name) {
    if ($Name -cnotmatch '^[a-z0-9_-]+\.json$') { throw 'Public proof output must be one explicit JSON leaf.' }
}

# Attempt each independent retirement once and preserve the original work cause.
function Invoke-ToolCleanup($Cause,[scriptblock[]]$Actions) {
    foreach ($action in $Actions) {
        try { & $action }
        catch {
            $next=$_.Exception
            $Cause=if ($null -eq $Cause) { $next } else { [AggregateException]::new($Cause,$next) }
        }
    }
    return $Cause
}
