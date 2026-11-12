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
/** Prove source-preserving mask edits, paired geometry, soft coverage and bounded filtering. */
import {
  editMaskInput,
  cropInputRaster,
} from '../../../frontend/comfyui/ui/input_canvas/domain/MaskInputEditing.js';
import {
  DEFAULT_MASK_EDITS,
  readMaskEditSettings,
} from '../../../frontend/comfyui/ui/input_canvas/domain/MaskEditSettings.js';
import {
  growMaskCoverage,
  featherMaskCoverage,
} from '../../../frontend/comfyui/ui/input_canvas/domain/MaskCoverageFilters.js';
import {
  decodeCanvasRaster,
  encodeCanvasRaster,
} from '../../../integrations/shared/CanvasRasterEncoding.js';
import {
  withInputBinding,
  readInputBinding,
} from '../../../frontend/comfyui/ui/input_canvas/infrastructure/InputBindingMetadata.js';

/** Build a deterministic interpreted source containing a soft edge and a hole. */
function source() {
  const bytes = new Uint8Array(8 * 6);
  bytes[2 * 8 + 2] = 64;
  bytes[2 * 8 + 3] = 255;
  bytes[3 * 8 + 4] = 128;
  return encodeCanvasRaster(8, 6, 'coverage8', bytes);
}

test('defaults preserve the full frame, holes and existing feather exactly', () => {
  const original = source(),
    edited = editMaskInput(original, DEFAULT_MASK_EDITS);
  expect(readMaskEditSettings(undefined)).toBe(DEFAULT_MASK_EDITS);
  expect(edited.cropped).toBe(false);
  expect(edited.bounds).toEqual({ left: 0, top: 0, width: 8, height: 6 });
  expect(edited.mask).toEqual(original);
  expect(edited.mapping.documentBounds).toEqual(edited.bounds);
});

test('workflow persistence retains validated edits with the source and accepts existing bindings without edits', () => {
  const binding = {
    version: 2 as const,
    field: 'image',
    files: [],
    selected: 'selection',
    entries: [
      {
        id: 'selection',
        label: 'Mask',
        source: {
          kind: 'canvas' as const,
          canvasId: 'editor',
          documentId: null,
          target: { kind: 'selection' as const },
        },
      },
    ],
  };
  const existing = withInputBinding({ unrelated: 42 }, 'mask', binding);
  expect(readInputBinding(existing, 'mask')).toEqual(binding);
  const edits = { ...DEFAULT_MASK_EDITS, feather: 8, crop_factor: 1.5 };
  const saved = withInputBinding(existing, 'mask', { ...binding, edits });
  expect(saved.unrelated).toBe(42);
  const restored = readInputBinding(
    withInputBinding(existing, 'mask', { ...binding, edits: { ...edits, mask_dilation: -58 } }),
    'mask',
  );
  expect(restored?.edits).toEqual({ ...edits, mask_dilation: -58 });
  expect(readInputBinding(JSON.parse(JSON.stringify(saved)), 'mask')?.edits).toEqual(edits);
  expect(() =>
    readInputBinding(
      withInputBinding({}, 'mask', {
        ...binding,
        edits: { ...edits, crop_factor: -1 },
      }),
      'mask',
    ),
  ).toThrow('Invalid mask edits');
});

test('grow changes crop support before geometry is frozen and post-crop shrink acts on prepared coverage', () => {
  const original = source();
  const grown = editMaskInput(original, {
    ...DEFAULT_MASK_EDITS,
    mask_dilation: 1,
    crop_factor: 1,
  });
  expect(grown.bounds).toEqual({ left: 1, top: 1, width: 5, height: 4 });
  const shrunk = editMaskInput(original, { ...DEFAULT_MASK_EDITS, post_dilation: -1 });
  expect(decodeCanvasRaster(shrunk.mask).every((value) => value === 0)).toBe(true);
});

test('extra feather starts from soft source values and never accumulates across slider changes', () => {
  const original = source(),
    settings = { ...DEFAULT_MASK_EDITS, feather: 1 };
  const first = editMaskInput(original, settings);
  editMaskInput(original, { ...settings, feather: 3 });
  expect(editMaskInput(original, settings)).toEqual(first);
  expect(editMaskInput(original, DEFAULT_MASK_EDITS).mask).toEqual(original);
  const result = decodeCanvasRaster(first.mask);
  expect(result[2 * 8 + 3]).toBe(Math.round((64 + 255 + 128) / 9));
  expect(result[1 * 8 + 2]).toBeGreaterThan(0);
  expect(result[2 * 8 + 3]).toBeLessThan(255);
});

test('crop factor preserves shaped soft coverage and records its original location for layer return', () => {
  const result = editMaskInput(source(), { ...DEFAULT_MASK_EDITS, crop_factor: 1 });
  expect(result.bounds).toEqual({ left: 2, top: 2, width: 3, height: 2 });
  expect(result.cropped).toBe(true);
  expect([...decodeCanvasRaster(result.mask)]).toEqual([64, 255, 0, 0, 0, 128]);
  expect(result.mapping).toEqual({
    imageSize: { width: 3, height: 2 },
    imageBounds: { left: 0, top: 0, width: 3, height: 2 },
    documentBounds: result.bounds,
  });
  const image = encodeCanvasRaster(
    8,
    6,
    'rgba8',
    Uint8Array.from({ length: 8 * 6 * 4 }, (_, i) => i),
  );
  const cropped = cropInputRaster(image, result.bounds);
  expect(cropped).toMatchObject({ width: 3, height: 2, format: 'rgba8' });
  expect([...decodeCanvasRaster(cropped)]).toEqual([
    ...decodeCanvasRaster(image).slice(72, 84),
    ...decodeCanvasRaster(image).slice(104, 116),
  ]);
});

test('context factor clamps to the source and shifting preserves the requested crop extent', () => {
  const original = source();
  expect(editMaskInput(original, { ...DEFAULT_MASK_EDITS, crop_factor: 2 }).bounds).toEqual({
    left: 0,
    top: 1,
    width: 6,
    height: 4,
  });
  expect(editMaskInput(original, { ...DEFAULT_MASK_EDITS, crop_factor: 100 }).bounds).toEqual({
    left: 0,
    top: 0,
    width: 8,
    height: 6,
  });
});

test('empty masks remain empty without inventing a crop or coverage', () => {
  const original = encodeCanvasRaster(3, 2, 'coverage8', new Uint8Array(6));
  const result = editMaskInput(original, { ...DEFAULT_MASK_EDITS, crop_factor: 1, feather: 2 });
  expect(result.cropped).toBe(false);
  expect(result.mask).toEqual(original);
});

test('grow/shrink preserve soft levels rather than binarizing the source', () => {
  const bytes = Uint8Array.from([0, 64, 128, 255, 0]);
  expect([...growMaskCoverage(bytes, 5, 1, 1)]).toEqual([64, 128, 255, 255, 255]);
  expect([...growMaskCoverage(bytes, 5, 1, -1)]).toEqual([0, 0, 64, 0, 0]);
  expect([...bytes]).toEqual([0, 64, 128, 255, 0]);
  expect([...featherMaskCoverage(new Uint8Array(6).fill(128), 3, 2, 512)]).toEqual([
    128, 128, 128, 128, 128, 128,
  ]);
});

test.each([-1, 0.5, Infinity, NaN, 101])('rejects unsupported crop factor %s', (crop_factor) => {
  expect(() => readMaskEditSettings({ ...DEFAULT_MASK_EDITS, crop_factor })).toThrow('Invalid');
});

test('rejects invalid filter settings and out-of-frame paired geometry', () => {
  for (const edits of [
    { mask_dilation: -513 },
    { mask_dilation: 513 },
    { post_dilation: 0.5 },
    { feather: -1 },
  ])
    expect(() => readMaskEditSettings({ ...DEFAULT_MASK_EDITS, ...edits })).toThrow('Invalid');
  expect(() => cropInputRaster(source(), { left: 7, top: 0, width: 2, height: 2 })).toThrow(
    'exceeds',
  );
});

test('linear filters match direct windows over non-square soft masks, including radii larger than the frame', () => {
  const width = 9,
    height = 5;
  const bytes = Uint8Array.from({ length: width * height }, (_, i) => (i * 83 + 17) % 256);
  for (const amount of [-12, -3, -1, 1, 3, 12]) {
    const radius = Math.abs(amount);
    const expected = Uint8Array.from(bytes, (_, index) => {
      const values: number[] = [],
        x = index % width,
        y = Math.floor(index / width);
      for (let j = Math.max(0, y - radius); j < Math.min(height, y + radius + 1); j++)
        for (let i = Math.max(0, x - radius); i < Math.min(width, x + radius + 1); i++)
          values.push(bytes[j * width + i] ?? 0);
      return amount > 0 ? Math.max(...values) : Math.min(...values);
    });
    expect(growMaskCoverage(bytes, width, height, amount)).toEqual(expected);
  }
});
