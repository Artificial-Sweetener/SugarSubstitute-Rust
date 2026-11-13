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
/** Write bounded GIMP distribution archives inside an explicitly selected artifact directory. */
import { execFileSync } from 'node:child_process';
import { mkdirSync, mkdtempSync, realpathSync, rmSync, writeFileSync, renameSync } from 'node:fs';
import path from 'node:path';
import type { GimpPackageContents } from './PackageContents.js';

/** Keep native package output on the workstation's artifact drive. */
export function admitPackageOutput(output: string): void {
  // Drive letters exist only in Windows path syntax, so judge the output by it on every host.
  if (!path.win32.isAbsolute(output) || path.win32.parse(output).root.toLowerCase() !== 'd:\\')
    throw new Error('Choose an absolute D-drive directory for GIMP package artifacts.');
}

/** Stage one exact layout and release only the staging directory created by this operation. */
export function writePackageArchive(
  root: string,
  output: string,
  payload: GimpPackageContents,
  directory: string,
  format: 'gex' | 'zip',
  development: boolean,
): string {
  admitPackageOutput(output);
  if (!/^[a-z][a-z0-9.-]+$/.test(directory)) throw new Error('Invalid GIMP package directory.');
  mkdirSync(output, { recursive: true });
  const staging = mkdtempSync(path.join(output, '.package-'));
  const owned = realpathSync(staging);
  if (!owned.startsWith(realpathSync(output) + path.sep))
    throw new Error('GIMP package staging escaped the selected artifact directory.');
  const target = path.join(
    output,
    `${payload.id}-${payload.version}-win64${development ? '-dev' : ''}.${format}`,
  );
  try {
    for (const [name, content] of payload.files) {
      const target = path.resolve(staging, directory, ...name.split('/'));
      if (!target.startsWith(path.join(staging, directory) + path.sep))
        throw new Error('A GIMP package input escaped its distribution directory.');
      mkdirSync(path.dirname(target), { recursive: true });
      writeFileSync(target, content);
    }
    const archive = path.join(staging, 'package.zip');
    execFileSync(
      path.join(root, '.venv/Scripts/python.exe'),
      ['-m', 'zipfile', '-c', archive, directory],
      { cwd: staging, windowsHide: true },
    );
    renameSync(archive, target);
  } catch (error: unknown) {
    try {
      removePackageStaging(staging, output);
    } catch (cleanup: unknown) {
      throw new AggregateError([error, cleanup], 'GIMP packaging and staging cleanup failed.');
    }
    throw error;
  }
  removePackageStaging(staging, output);
  return target;
}

/** Revalidate the exact owned directory before recursive artifact cleanup. */
function removePackageStaging(staging: string, output: string): void {
  if (!realpathSync(staging).startsWith(realpathSync(output) + path.sep))
    throw new Error('GIMP package cleanup escaped its owned artifact directory.');
  rmSync(staging, { recursive: true, force: true });
}
