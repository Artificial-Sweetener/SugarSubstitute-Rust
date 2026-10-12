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

import { decodeCanvasSurfaceGeometry } from '../../../frontend/comfyui/ui/output_canvas/infrastructure/CanvasGeometryDecoder.js';
import type { CanvasSurfaceGeometry } from '../../../frontend/comfyui/ui/output_canvas/domain/CanvasViewPort.js';
import { canvasSnapshot } from '../support/CanvasPortFixture.js';

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

test.each([
  { ...geometry, units: 'physical-pixels' },
  { ...geometry, coordinateSpace: 'document' },
  { ...geometry, physicalPixelsPerCssPixel: 0 },
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
