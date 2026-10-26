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
/** Generate a GIMP extension archive without writing into an installed GIMP profile. */
import { execFileSync } from 'node:child_process';
import {
  mkdirSync,
  mkdtempSync,
  readFileSync,
  realpathSync,
  rmSync,
  writeFileSync,
  renameSync,
} from 'node:fs';
import path from 'node:path';
import { pathToFileURL } from 'node:url';
import { packagePayload, packageSources } from './PackageContents.js';

/** Use Python's ZIP implementation while retaining package selection, paths and lifecycle in the declared build owner. */
export function buildGimpPackage(root: string, output: string, development: boolean): string {
  if (!path.isAbsolute(output) || path.parse(output).root.toLowerCase() !== 'd:\\')
    throw new Error('Choose an absolute D-drive directory for GIMP package artifacts.');
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
  mkdirSync(output, { recursive: true });
  const staging = mkdtempSync(path.join(output, '.package-'));
  if (!realpathSync(staging).startsWith(realpathSync(output) + path.sep))
    throw new Error('GIMP package staging escaped the selected artifact directory.');
  try {
    for (const [name, content] of payload.files) {
      const target = path.join(staging, payload.id, ...name.split('/'));
      mkdirSync(path.dirname(target), { recursive: true });
      writeFileSync(target, content);
    }
    const archive = path.join(staging, 'package.zip');
    execFileSync(
      path.join(root, '.venv/Scripts/python.exe'),
      ['-m', 'zipfile', '-c', archive, payload.id],
      {
        cwd: staging,
        windowsHide: true,
        timeout: 120_000,
      },
    );
    const target = path.join(
      output,
      `${payload.id}-${payload.version}-win64${development ? '-dev' : ''}.gex`,
    );
    renameSync(archive, target);
    return target;
  } finally {
    rmSync(staging, { recursive: true, force: true });
  }
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
  console.log(buildGimpPackage(root, output, process.argv.includes('--development')));
}

if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) main();
