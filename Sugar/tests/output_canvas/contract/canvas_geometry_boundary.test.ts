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
/** Validate geometry units, density and attachment scope independently of a renderer. */

import { readFileSync } from 'node:fs';
import { decodeCanvasSurfaceGeometry } from '../../../frontend/comfyui/ui/output_canvas/infrastructure/CanvasGeometryDecoder.js';
import type { CanvasSurfaceGeometry } from '../../../frontend/comfyui/ui/output_canvas/domain/CanvasViewPort.js';
import { isCanvasSurfaceGeometry } from '../../../integrations/shared/CanvasSurfaceValidation.generated.js';
import { canvasSnapshot } from '../support/CanvasPortFixture.js';

const vectors = JSON.parse(
  readFileSync('tests/output_canvas/support/canvas_surface_geometry_vectors.json', 'utf8'),
) as {
  base: Record<string, unknown>;
  cases: { name: string; path?: string; value?: unknown; remove?: boolean; valid: boolean }[];
};

const address = canvasSnapshot('geometry').address;
const authority = { address, origin: 'https://substitute.example' };
const geometry: CanvasSurfaceGeometry = {
  coordinateSpace: 'client',
  units: 'css-pixels',
  origin: { x: -100, y: 50 },
  viewport: { width: 800, height: 600 },
  clip: { x: 0, y: 0, width: 800, height: 600 },
  physicalPixelsPerCssPixel: 1.5,
  visible: true,
  focused: false,
};

test('preserves explicit density and units while copying immutable geometry', () => {
  const decoded = decodeCanvasSurfaceGeometry(
    { schema: 1, address, geometry },
    authority.origin,
    authority,
  );
  expect(decoded).toEqual(geometry);
  expect(decoded).not.toBe(geometry);
  expect(decoded?.viewport).not.toBe(geometry.viewport);
  expect(Object.isFrozen(decoded?.viewport)).toBe(true);
  expect(
    decodeCanvasSurfaceGeometry(
      {
        schema: 1,
        address,
        geometry: { ...geometry, viewport: { width: 0, height: 0 }, visible: false },
      },
      authority.origin,
      authority,
    )?.visible,
  ).toBe(false);
});

test('retains immutable native material regions independently of gesture occlusions', () => {
  const backdrops = [{ x: 8, y: 550, width: 200, height: 42, cornerRadius: 8, sigma: 10 }];
  const decoded = decodeCanvasSurfaceGeometry(
    { schema: 1, address, geometry: { ...geometry, backdrops } },
    authority.origin,
    authority,
  );
  expect(decoded?.backdrops).toEqual(backdrops);
  expect(decoded?.backdrops).not.toBe(backdrops);
  expect(Object.isFrozen(decoded?.backdrops?.[0])).toBe(true);
  expect(decoded?.occlusions).toBeUndefined();
});

test.each([
  { ...geometry, units: 'physical-pixels' },
  { ...geometry, coordinateSpace: 'document' },
  { ...geometry, physicalPixelsPerCssPixel: 0 },
  { ...geometry, physicalPixelsPerCssPixel: -0.01 },
  { ...geometry, physicalPixelsPerCssPixel: 64.01 },
  { ...geometry, physicalPixelsPerCssPixel: Infinity },
  { ...geometry, viewport: { width: -1, height: 600 } },
  { ...geometry, viewport: { width: 1e9, height: 600 } },
  { ...geometry, origin: { x: NaN, y: 0 } },
  { ...geometry, clip: { x: 0, y: 0, width: 10, height: -1 } },
  { ...geometry, visible: 'true' },
  { ...geometry, focused: undefined },
  null,
])('rejects malformed geometry %#', (external) => {
  expect(
    decodeCanvasSurfaceGeometry(
      { schema: 1, address, geometry: external },
      authority.origin,
      authority,
    ),
  ).toBeNull();
});

test('normalizes extra fields while keeping only the shared surface geometry', () => {
  const extended = {
    ...geometry,
    unrelated: true,
    origin: { ...geometry.origin, unrelated: true },
  };
  expect(isCanvasSurfaceGeometry(geometry)).toBe(true);
  expect(isCanvasSurfaceGeometry(extended)).toBe(false);
  expect(
    decodeCanvasSurfaceGeometry(
      { schema: 1, address, geometry: extended },
      authority.origin,
      authority,
    ),
  ).toEqual(geometry);
});

test('admits the exact density boundary and small positive finite scales', () => {
  for (const physicalPixelsPerCssPixel of [Number.MIN_VALUE, 64]) {
    expect(
      decodeCanvasSurfaceGeometry(
        { schema: 1, address, geometry: { ...geometry, physicalPixelsPerCssPixel } },
        authority.origin,
        authority,
      )?.physicalPixelsPerCssPixel,
    ).toBe(physicalPixelsPerCssPixel);
  }
});

test('rejects unauthorized or retired attachment updates', () => {
  const message = { schema: 1, address, geometry };
  expect(decodeCanvasSurfaceGeometry(message, 'https://other.example', authority)).toBeNull();
  expect(
    decodeCanvasSurfaceGeometry(
      { ...message, address: { ...address, generation: 2 } },
      authority.origin,
      authority,
    ),
  ).toBeNull();
});

test.each(vectors.cases)('matches the shared Rust/web geometry vector: $name', (vector) => {
  const candidate = JSON.parse(JSON.stringify(vectors.base)) as Record<string, unknown>;
  if (vector.path) {
    const parts = vector.path.split('.');
    let parent = candidate;
    for (const part of parts.slice(0, -1)) parent = parent[part] as Record<string, unknown>;
    const key = parts.at(-1);
    if (!key) throw new Error('Geometry vector path is empty.');
    if (vector.remove) Reflect.deleteProperty(parent, key);
    else if ('value' in vector) parent[key] = vector.value;
  }
  expect(isCanvasSurfaceGeometry(candidate)).toBe(vector.valid);
});
