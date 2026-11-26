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
/**
 * Build the prompt editor's spellchecker module from `spellcheck/` and place it with the static
 * inputs the web build deploys. The crate pins its toolchain and the lock file pins spellbook, so
 * the module changes only when its source or those pins do.
 */
import { spawnSync } from 'node:child_process';
import { copyFileSync, mkdirSync } from 'node:fs';
import path from 'node:path';

const root = path.resolve(import.meta.dirname, '../..');
const crate = path.join(root, 'spellcheck');
const targetDirectory = process.env.CARGO_TARGET_DIR ?? path.join(crate, 'target');
const result = spawnSync(
  'cargo',
  ['build', '--locked', '--release', '--target', 'wasm32-unknown-unknown'],
  { cwd: crate, stdio: 'inherit', env: { ...process.env, CARGO_TARGET_DIR: targetDirectory } },
);
if (result.status !== 0) process.exit(result.status ?? 1);
const output = path.join(root, 'frontend/static/spellcheck/sugarcubes-spellcheck.wasm');
mkdirSync(path.dirname(output), { recursive: true });
copyFileSync(
  path.join(targetDirectory, 'wasm32-unknown-unknown/release/sugarcubes_spellcheck.wasm'),
  output,
);
console.log(`Spellchecker module written to ${path.relative(root, output)}`);
