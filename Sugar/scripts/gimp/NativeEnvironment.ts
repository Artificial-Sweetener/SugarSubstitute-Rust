//    SugarCubes - composable workflow units for ComfyUI
//    Copyright (C) 2026  Artificial Sweetener and contributors
//
//    This program is free software: you can redistribute it and/or modify
//    it under the terms of the GNU Affero General Public License as published by
//    the Free Software Foundation, either version 3 of the License, or
//    (at your option) any later version.
//
//    This program is distributed in the hope that it will be useful,
//    but WITHOUT ANY WARRANTY; without even the implied warranty of
//    MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
//    GNU Affero General Public License for more details.
//
//    You should have received a copy of the GNU Affero General Public License
//    along with this program.  If not, see <https://www.gnu.org/licenses/>.
/** Select VS2022 Community and the pinned Windows SDK explicitly for fresh native builds. */
import { execFileSync } from 'node:child_process';

/** Initialize the selected developer shell without exposing environment values in diagnostics. */
export function nativeEnvironment(): NodeJS.ProcessEnv {
  if (process.platform !== 'win32') return process.env;
  const product = process.env.SUGARCUBES_VS_PRODUCT ?? 'Microsoft.VisualStudio.Product.Community';
  if (
    product !== 'Microsoft.VisualStudio.Product.Community' &&
    product !== 'Microsoft.VisualStudio.Product.Enterprise'
  )
    throw new Error('Select the declared VS2022 Community or Enterprise product.');
  const vsRoot = execFileSync(
    'C:/Program Files (x86)/Microsoft Visual Studio/Installer/vswhere.exe',
    ['-products', product, '-version', '[17.0,18.0)', '-property', 'installationPath'],
    { encoding: 'utf8' },
  ).trim();
  if (!vsRoot || vsRoot.includes('\n') || vsRoot.includes("'"))
    throw new Error('Select exactly one installation of the declared VS2022 product.');
  const module = `${vsRoot}/Common7/Tools/Microsoft.VisualStudio.DevShell.dll`;
  const script = `$ErrorActionPreference='Stop'; Import-Module -Name '${module}'; Enter-VsDevShell -VsInstallPath '${vsRoot}' -SkipAutomaticLocation -DevCmdArguments '-arch=x64 -host_arch=x64 -winsdk=10.0.26100.0' | Out-Null; $values=@{}; Get-ChildItem Env: | ForEach-Object { $values[$_.Name]=$_.Value }; $values | ConvertTo-Json -Compress`;
  const output = execFileSync(
    'powershell.exe',
    ['-NoProfile', '-NonInteractive', '-Command', script],
    { encoding: 'utf8', windowsHide: true },
  );
  const value: unknown = JSON.parse(output);
  if (!value || typeof value !== 'object' || Array.isArray(value))
    throw new Error('The native build environment is invalid.');
  const environment: NodeJS.ProcessEnv = { ...process.env };
  for (const [key, item] of Object.entries(value)) {
    if (typeof item !== 'string')
      throw new Error('The native build environment contains an invalid entry.');
    environment[key] = item;
  }
  if (
    !environment.VCToolsInstallDir ||
    !environment.WindowsSdkDir ||
    !environment.INCLUDE ||
    !environment.LIB
  )
    throw new Error('VS2022 and Windows SDK initialization is incomplete.');
  return environment;
}
