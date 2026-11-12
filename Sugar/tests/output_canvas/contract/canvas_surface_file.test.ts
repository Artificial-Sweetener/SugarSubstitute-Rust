/** @jest-environment node */
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
/** Qualify the renderer-neutral file reference without granting filesystem authority. */

import { isCanvasSurfaceFileReference } from '../../../integrations/shared/CanvasSurfaceValidation.generated.js';
import { readFileSync } from 'node:fs';

const file = { filename: 'résumé🌈.png', subfolder: 'batch/one', type: 'output' };
const vectors = JSON.parse(
  readFileSync('integrations/contracts/canvas_surface_file_vectors.json', 'utf8'),
) as { name: string; value: unknown; valid: boolean }[];

test.each(vectors)('matches the shared Rust/web file reference vector: $name', (vector) => {
  expect(isCanvasSurfaceFileReference(vector.value)).toBe(vector.valid);
});

test('accepts bounded Unicode references and only the declared Comfy storage kinds', () => {
  for (const type of ['input', 'output', 'temp']) {
    expect(isCanvasSurfaceFileReference({ ...file, type })).toBe(true);
  }
  expect(isCanvasSurfaceFileReference({ ...file, filename: '🌈'.repeat(4096) })).toBe(true);
  expect(isCanvasSurfaceFileReference({ ...file, filename: '🌈'.repeat(4097) })).toBe(false);
});

test.each([
  { ...file, type: 'other' },
  { ...file, filename: '\ud800' },
  { ...file, subfolder: '\udc00' },
  { ...file, subfolder: 'x'.repeat(4097) },
  { ...file, filename: 'x'.repeat(100000) },
  { ...file, path: 'foreign' },
  { filename: file.filename, type: file.type },
  null,
])('rejects malformed or extended file reference %#', (value) => {
  expect(isCanvasSurfaceFileReference(value)).toBe(false);
});
