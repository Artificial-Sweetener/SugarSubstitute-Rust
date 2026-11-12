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
/** Inspect explicitly selected developer prerequisites without changing Adobe installation state. */
import { existsSync, statSync } from 'node:fs';
import path from 'node:path';
import { createRequire } from 'node:module';

const require = createRequire(import.meta.url);
for (const name of ['typescript', 'esbuild', '@adobe/cc-ext-uxp-types', '@types/photoshop']) {
  require.resolve(name === '@types/photoshop' ? `${name}/index.d.ts` : name);
  console.log(`Available: ${name}`);
}
const argument = process.argv.indexOf('--executable');
const executable =
  argument >= 0 ? process.argv[argument + 1] : process.env.SUGARCUBES_PHOTOSHOP_EXECUTABLE;
if (argument >= 0 && !executable) throw new Error('--executable requires an absolute path.');
if (executable) {
  if (!path.isAbsolute(executable) || !existsSync(executable) || !statSync(executable).isFile())
    throw new Error('The selected Photoshop executable does not exist.');
  console.log('The explicitly selected Photoshop executable exists.');
} else
  console.log(
    'Set SUGARCUBES_PHOTOSHOP_EXECUTABLE or pass --executable to verify a host installation.',
  );
console.log('Use UXP Developer Tool to verify Developer Mode, host loading and debugging.');
