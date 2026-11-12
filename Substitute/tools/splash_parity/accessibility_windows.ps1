# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (C) 2026 Artificial Sweetener and contributors
param(
    [Parameter(Mandatory = $true)][Int64]$WindowHandle,
    [Parameter(Mandatory = $true)][Int32]$ProcessId,
    [ValidateSet('Inspect', 'Focus', 'Invoke')][string]$Action = 'Inspect',
    [string]$ControlName = ''
)

$ErrorActionPreference = 'Stop'
[Console]::OutputEncoding = [System.Text.UTF8Encoding]::new($false)
Add-Type -AssemblyName UIAutomationClient
Add-Type -AssemblyName UIAutomationTypes
Add-Type -AssemblyName UIAutomationClientsideProviders
$providerReferences = @(
    [System.Windows.Automation.AutomationElement].Assembly.Location,
    [UIAutomationClientsideProviders.UIAutomationClientSideProviders].Assembly.Location
)
Add-Type -ReferencedAssemblies $providerReferences -TypeDefinition @"
using System.Runtime.CompilerServices;
public static class SugarSubstituteObservationProviders {
    [MethodImpl(MethodImplOptions.NoInlining)]
    public static void Initialize() {
        System.Windows.Automation.ClientSettings.RegisterClientSideProviders(
            UIAutomationClientsideProviders.UIAutomationClientSideProviders.ClientSideProviderDescriptionTable);
    }
}
"@
# The managed provider loader examines its caller's reflected type. Keep a real
# compiled frame so PowerShell's dynamic invocation cannot suppress standard proxies.
[SugarSubstituteObservationProviders]::Initialize()
$root = [System.Windows.Automation.AutomationElement]::FromHandle([IntPtr]$WindowHandle)
if ($null -eq $root -or $root.Current.ProcessId -ne $ProcessId) {
    throw 'The qualified accessibility root has changed ownership.'
}
$buttons = $root.FindAll(
    [System.Windows.Automation.TreeScope]::Descendants,
    [System.Windows.Automation.PropertyCondition]::new(
        [System.Windows.Automation.AutomationElement]::ControlTypeProperty,
        [System.Windows.Automation.ControlType]::Button
    )
)
if ($Action -ne 'Inspect') {
    $matches = @($buttons | Where-Object {
        $_.Current.Name -eq $ControlName -and $_.Current.NativeWindowHandle -ne 0 -and
        $_.Current.AutomationId -in @('1', '2')
    })
    if ($matches.Count -ne 1 -or $matches[0].Current.ProcessId -ne $ProcessId -or
        $root.Current.ProcessId -ne $ProcessId) {
        throw 'Accessible action must identify exactly one currently owned caption button.'
    }
    if ($Action -eq 'Focus') {
        $matches[0].SetFocus()
        $focusObservation = [System.Diagnostics.Stopwatch]::StartNew()
        while (-not $matches[0].Current.HasKeyboardFocus -and $focusObservation.ElapsedMilliseconds -lt 1000) {
            Start-Sleep -Milliseconds 5
        }
        if (-not $matches[0].Current.HasKeyboardFocus) {
            throw 'The accessible caption button did not receive keyboard focus.'
        }
    }
    else {
        $invocation = $matches[0].GetCurrentPattern([System.Windows.Automation.InvokePattern]::Pattern)
    }
}
$result = @()
foreach ($button in $buttons) {
    $current = $button.Current
    if ($current.ProcessId -ne $ProcessId) {
        throw 'A foreign process owns a qualified accessibility control.'
    }
    $pattern = $null
    $invokable = $button.TryGetCurrentPattern(
        [System.Windows.Automation.InvokePattern]::Pattern,
        [ref]$pattern
    )
    $bounds = $current.BoundingRectangle
    $result += [ordered]@{
        name = $current.Name
        automation_id = $current.AutomationId
        process_id = $current.ProcessId
        control_type = $current.ControlType.ProgrammaticName
        keyboard_focusable = $current.IsKeyboardFocusable
        has_keyboard_focus = $current.HasKeyboardFocus
        window_handle = $current.NativeWindowHandle
        enabled = $current.IsEnabled
        offscreen = $current.IsOffscreen
        invoke_supported = $invokable
        bounds = @($bounds.X, $bounds.Y, $bounds.Width, $bounds.Height)
    }
}
ConvertTo-Json -Depth 5 -InputObject @($result) -Compress
if ($Action -eq 'Invoke') {
    if ($matches[0].Current.ProcessId -ne $ProcessId -or $root.Current.ProcessId -ne $ProcessId) {
        throw 'The accessible invocation target has changed ownership.'
    }
    $invocation.Invoke()
}
