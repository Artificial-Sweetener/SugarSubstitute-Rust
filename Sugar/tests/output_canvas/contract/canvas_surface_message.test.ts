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
/** Keep result and attachment admission identical across native and web hosts. */

import { readFileSync } from 'node:fs';
import { isCanvasMessage } from '../../../integrations/shared/CanvasValidation.generated.js';

const vectors = JSON.parse(
  readFileSync('integrations/contracts/canvas_surface_message_vectors.json', 'utf8'),
) as {
  base: Record<string, unknown>;
  cases: { name: string; path?: string; value?: unknown; remove?: boolean; valid: boolean }[];
};

test.each(vectors.cases)('shared surface message: $name', (vector) => {
  const value = structuredClone(vectors.base);
  if (vector.path) {
    const parts = vector.path.split('.');
    const key = parts.pop();
    if (!key) throw new Error('Invalid vector path.');
    let target = value;
    for (const part of parts) target = target[part] as Record<string, unknown>;
    if (vector.remove) Reflect.deleteProperty(target, key);
    else target[key] = vector.value;
  }
  expect(isCanvasMessage(value)).toBe(vector.valid);
});

test('rejects an unpaired surrogate in a result identity', () => {
  expect(isCanvasMessage({ ...vectors.base, itemId: 'result:\ud800' })).toBe(false);
});
