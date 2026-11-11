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
/** Resolve the reviewed browser-safe PNG decoder and ship its exact license notices. */
import { copyFileSync, mkdirSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { createRequire } from 'node:module';
import type { Metafile } from 'esbuild';

const resolve = createRequire(import.meta.url).resolve;
const dependencies: ReadonlyMap<string, string> = new Map([
  ['upng-js', resolve('upng-js')],
  ['pako', resolve('pako')],
]);

/** Admit exact reviewed packages while leaving all other native dependencies forbidden. */
export function nativeImageDependency(name: string): string | null {
  return dependencies.get(name) ?? null;
}

/** Include attribution only when the native package actually contains the PNG decoder. */
export function packageNativeImageLicenses(staging: string, metafile: Metafile): void {
  if (
    !Object.keys(metafile.inputs).some((input) => input.replaceAll('\\', '/').includes('/upng-js/'))
  )
    return;
  const source = fileURLToPath(new URL('../../third_party/', import.meta.url));
  const target = path.join(staging, 'third_party');
  mkdirSync(path.join(target, 'licenses'), { recursive: true });
  for (const name of ['upng-js.MIT.txt', 'pako.LICENSE.txt'])
    copyFileSync(path.join(source, 'licenses', name), path.join(target, 'licenses', name));
  copyFileSync(path.join(source, 'NOTICE.md'), path.join(target, 'NOTICE.md'));
}
