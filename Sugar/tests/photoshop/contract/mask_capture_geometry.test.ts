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
/** Prove native cropped selections retain their scale and position inside the entire canvas frame. */
import { jest } from '@jest/globals';
import { build } from 'esbuild';
import { runInNewContext } from 'node:vm';
import { decodeMaskPixels } from '../../../integrations/shared/CanvasMaskPixels.js';
import type { CanvasMaskPixels } from '../../../integrations/contracts/CanvasMasksProtocol.generated.js';
import { BUILD_FIXTURE_SETUP_TIMEOUT_MS } from '../../frontend_core/support/BuildFixtureBudget.js';

interface Bounds {
  readonly left: number;
  readonly top: number;
  readonly right: number;
  readonly bottom: number;
}
interface SelectionOptions {
  readonly sourceBounds: Bounds;
  readonly targetSize?: { readonly width: number; readonly height: number };
}
let code = '';
beforeAll(async () => {
  const bundle = await build({
    entryPoints: ['integrations/photoshop/src/infrastructure/PhotoshopSelectionPixels.ts'],
    bundle: true,
    write: false,
    format: 'cjs',
    platform: 'neutral',
    target: 'es2022',
    external: ['photoshop'],
  });
  const file = bundle.outputFiles[0];
  if (!file) throw new Error('Missing native selection capture bundle.');
  code = file.text;
}, BUILD_FIXTURE_SETUP_TIMEOUT_MS);

/** Model Photoshop's observed crop-then-resize behavior at its native API boundary. */
function fixture(width: number, height: number, bounds: Bounds | null) {
  const dispose = jest.fn(),
    getSelection = jest.fn(async (options: SelectionOptions) => {
      if (!bounds) throw new Error('An empty selection must not request pixels.');
      const source = {
        left: Math.max(0, bounds.left),
        top: Math.max(0, bounds.top),
        right: Math.min(width, bounds.right),
        bottom: Math.min(height, bounds.bottom),
      };
      const cropWidth = options.targetSize?.width ?? source.right - source.left,
        cropHeight = options.targetSize?.height ?? source.bottom - source.top;
      return {
        sourceBounds: source,
        imageData: {
          width: cropWidth,
          height: cropHeight,
          components: 1,
          componentSize: 8,
          getData: async () => new Uint8Array(cropWidth * cropHeight).fill(255),
          dispose,
        },
      };
    });
  const module: { exports: Record<string, unknown> } = { exports: {} };
  runInNewContext(code, {
    module,
    require: () => ({ imaging: { getSelection } }),
  });
  // The compiled native adapter has an isolated type identity across the VM boundary.
  const read = module.exports.readSelectionPixels as (
    document: unknown,
    maximumSize: number,
  ) => Promise<CanvasMaskPixels>;
  return {
    read: (maximumSize: number) =>
      read({ id: 1, width, height, selection: { bounds } }, maximumSize),
    getSelection,
    dispose,
  };
}

test.each([
  [1152, 2048, { left: 300, top: 400, right: 700, bottom: 900 }],
  [2048, 1152, { left: 800, top: 300, right: 1200, bottom: 700 }],
])(
  'fits an off-center selection into the full %s by %s document at thumbnail scale',
  async (width, height, bounds) => {
    const f = fixture(width, height, bounds),
      result = await f.read(288),
      pixels = decodeMaskPixels(result),
      sx = result.width / width,
      sy = result.height / height;
    expect(result.width).toBe(Math.round(width * Math.min(1, 288 / Math.max(width, height))));
    expect(result.height).toBe(Math.round(height * Math.min(1, 288 / Math.max(width, height))));
    const left = Math.round(bounds.left * sx),
      top = Math.round(bounds.top * sy),
      right = Math.round(bounds.right * sx),
      bottom = Math.round(bounds.bottom * sy);
    for (let y = 0; y < result.height; y++)
      for (let x = 0; x < result.width; x++)
        expect(pixels[y * result.width + x]).toBe(
          x >= left && x < right && y >= top && y < bottom ? 255 : 0,
        );
    expect(f.getSelection).toHaveBeenCalledWith(
      expect.objectContaining({
        targetSize: { width: right - left, height: bottom - top },
      }),
    );
    expect(f.dispose).toHaveBeenCalledTimes(1);
  },
);

test('keeps full-resolution pixels at their authored canvas coordinates', async () => {
  const f = fixture(48, 32, { left: 9, top: 7, right: 17, bottom: 11 }),
    result = await f.read(0),
    pixels = decodeMaskPixels(result);
  expect([result.width, result.height]).toEqual([48, 32]);
  expect(pixels.filter((value) => value === 255)).toHaveLength(32);
  expect(pixels[7 * 48 + 9]).toBe(255);
  expect(pixels[11 * 48 + 17]).toBe(0);
  expect(f.getSelection.mock.calls[0]?.[0].targetSize).toBeUndefined();
});

test('keeps an empty selection as an empty full-document thumbnail', async () => {
  const f = fixture(1152, 2048, null),
    result = await f.read(288);
  expect([result.width, result.height]).toEqual([162, 288]);
  expect(decodeMaskPixels(result).some((value) => value !== 0)).toBe(false);
  expect(f.getSelection).not.toHaveBeenCalled();
});
