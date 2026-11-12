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
/** Keep native and web attachment identities on the same bounded wire contract. */
import { readFileSync } from 'node:fs';
import { expect, test } from '@jest/globals';
import { isCanvasSurfaceAddress } from '../../../integrations/shared/CanvasSurfaceValidation.generated.js';
import { canvasBindings } from '../../../scripts/integrations/generate-canvas-contract.js';

const vectors = JSON.parse(
  readFileSync('integrations/contracts/canvas_surface_address_vectors.json', 'utf8'),
) as {
  base: Record<string, unknown>;
  cases: { name: string; path?: string; value?: unknown; remove?: boolean; valid: boolean }[];
};

test.each(vectors.cases)('shared surface address: $name', (vector) => {
  const value = { ...vectors.base };
  if (vector.path) {
    if (vector.remove) Reflect.deleteProperty(value, vector.path);
    else value[vector.path] = vector.value;
  }
  expect(isCanvasSurfaceAddress(value)).toBe(vector.valid);
});

test('rejects unpaired UTF-16 surrogates before transport', () => {
  expect(isCanvasSurfaceAddress({ ...vectors.base, viewId: 'view:\ud800' })).toBe(false);
});

test('generates the Rust identity guard from the same schema and rejects unsupported changes', async () => {
  const source = readFileSync('integrations/contracts/canvas.schema.json', 'utf8');
  const bindings = await canvasBindings(source);
  expect(bindings['integrations/contracts/CanvasSurfaceProtocol.generated.rs']).toBe(
    readFileSync('integrations/contracts/CanvasSurfaceProtocol.generated.rs', 'utf8').replaceAll(
      '\r\n',
      '\n',
    ),
  );
  const schema = JSON.parse(source) as {
    $defs: { CanvasSurfaceAddress: { properties: { viewId: { pattern: string } } } };
  };
  schema.$defs.CanvasSurfaceAddress.properties.viewId.pattern = '^view:.*$';
  await expect(canvasBindings(JSON.stringify(schema))).rejects.toThrow();
});
