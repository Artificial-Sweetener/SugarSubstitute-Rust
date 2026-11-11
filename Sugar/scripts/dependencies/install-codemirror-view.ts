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
/** npm lifecycle and verification entrypoint for the sealed CodeMirror view patch. */
import { fileURLToPath } from 'node:url';
import { installCodeMirrorPatch } from './codemirror-view-patch.js';

const args = process.argv.slice(2);
if (args.length > 1 || (args.length === 1 && args[0] !== '--check'))
  throw new Error('CM_PATCH_ARGUMENTS');
const packageRoot = fileURLToPath(new URL('../../node_modules/@codemirror/view/', import.meta.url));
const result = installCodeMirrorPatch(packageRoot, args[0] === '--check' ? 'check' : 'apply');
process.stdout.write(`CodeMirror view patch: ${result}\n`);
