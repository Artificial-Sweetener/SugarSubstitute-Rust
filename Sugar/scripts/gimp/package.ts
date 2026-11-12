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
/** Generate standard plug-in and extension archives without changing an installed GIMP profile. */
import { execFileSync } from 'node:child_process';
import { readFileSync } from 'node:fs';
import path from 'node:path';
import { pathToFileURL } from 'node:url';
import { GIMP_PLUGIN_DIRECTORY, packagePayload, packageSources } from './PackageContents.js';
import { pluginPackageContents } from './PluginPackageContents.js';
import { admitPackageOutput, writePackageArchive } from './PackageArchive.js';

/** Build the native payload once and package both supported distribution layouts. */
export function buildGimpPackages(
  root: string,
  output: string,
  development: boolean,
): readonly string[] {
  admitPackageOutput(output);
  execFileSync(
    process.execPath,
    [
      '--import',
      'tsx',
      path.join(root, 'scripts/gimp/native.ts'),
      'build',
      ...(development ? [] : ['--release']),
    ],
    { cwd: root, stdio: 'inherit', windowsHide: true },
  );
  const executable = readFileSync(
    path.join(
      root,
      'build/gimp/native',
      development ? 'debug' : 'release',
      'sugarcubes-gimp-companion.exe',
    ),
  );
  const payload = packagePayload(root, executable, development);
  const plugin = pluginPackageContents(payload, development);
  return [
    writePackageArchive(root, output, plugin, GIMP_PLUGIN_DIRECTORY, 'zip', development),
    writePackageArchive(root, output, payload, payload.id, 'gex', development),
  ];
}

/** Validate authored package inputs separately from native compilation, or build the selected binary package. */
function main(): void {
  const root = path.resolve(import.meta.dirname, '../..');
  if (process.argv.includes('--check')) {
    const source = packageSources(root);
    if ([...source.files.values()].some((content) => !content.length))
      throw new Error('Empty GIMP package input.');
    console.log('GIMP extension inputs validated.');
    return;
  }
  const outputIndex = process.argv.indexOf('--output');
  const output = outputIndex < 0 ? undefined : process.argv[outputIndex + 1];
  if (!output) throw new Error('Pass --output with the D-drive package artifact directory.');
  console.log(buildGimpPackages(root, output, process.argv.includes('--development')).join('\n'));
}

if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) main();
