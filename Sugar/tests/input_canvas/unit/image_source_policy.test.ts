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
/** Prove image-field discovery, persistent identity and image selection content without native editor coupling. */
import { recognizeInputLoaders } from '../../../frontend/comfyui/ui/input_canvas/domain/InputLoader.js';
import {
  readInputBinding,
  withInputBinding,
} from '../../../frontend/comfyui/ui/input_canvas/infrastructure/InputBindingMetadata.js';
import { canvasSelectionImage } from '../../../frontend/comfyui/ui/input_canvas/domain/CanvasSelectionImage.js';
import {
  encodeCanvasRaster,
  decodeCanvasRaster,
} from '../../../integrations/shared/CanvasRasterEncoding.js';
import { sourceCoverage, sourceRgba } from '../support/source_mask_fixture.js';
import type { InputBinding } from '../../../frontend/comfyui/ui/input_canvas/domain/InputBinding.js';

test('recognizes mixed IMAGE/MASK loaders, nonstandard image outputs and independently uploaded fields', () => {
  const definition = {
    classType: 'CustomSource',
    outputs: ['IMAGE', 'MASK'],
    channel: null,
    fields: [
      { name: 'reference', upload: true, folder: 'input', ordered: false },
      { name: 'guides', upload: true, folder: 'output', ordered: true },
      { name: 'image', upload: false, folder: null, ordered: false },
    ],
  };
  expect(recognizeInputLoaders(definition)).toEqual([
    { kind: 'image', classType: 'CustomSource', field: 'reference', ordered: false, channel: null },
    { kind: 'image', classType: 'CustomSource', field: 'guides', ordered: true, channel: null },
  ]);
  expect(recognizeInputLoaders({ ...definition, outputs: ['UME_IMAGE'] })).toHaveLength(2);
  expect(recognizeInputLoaders({ ...definition, fields: [] })).toEqual([]);
  expect(
    recognizeInputLoaders({
      classType: 'SimpleSyrup.LoadImageList',
      outputs: ['IMAGE'],
      fields: [],
      channel: null,
    }),
  ).toMatchObject([{ kind: 'image', ordered: true }]);
});

test('retains image colors, full-canvas geometry and feathered selection alpha instead of returning a mask picture', () => {
  const image = canvasSelectionImage(sourceRgba, sourceCoverage);
  expect([image.width, image.height, image.format]).toEqual([3, 2, 'rgba8']);
  expect([...decodeCanvasRaster(image)]).toEqual([
    0, 0, 0, 0, 40, 50, 60, 127, 0, 0, 0, 0, 100, 110, 120, 63, 0, 0, 0, 0, 0, 0, 0, 0,
  ]);
  expect(() => canvasSelectionImage(sourceCoverage, sourceRgba)).toThrow('full-canvas frame');
  expect(() =>
    canvasSelectionImage(sourceRgba, encodeCanvasRaster(1, 1, 'coverage8', new Uint8Array([255]))),
  ).toThrow('full-canvas frame');
});

test('keeps independently bound image fields and public mask metadata intact during persistence and restore', () => {
  const binding: InputBinding = {
    version: 2,
    field: 'reference',
    files: [],
    selected: 'a',
    entries: [
      {
        id: 'a',
        label: 'Image 1',
        source: {
          kind: 'canvas',
          canvasId: 'photoshop',
          documentId: null,
          target: { kind: 'composition' },
        },
      },
    ],
  };
  const original = { unrelated: 'keep', sugarcubes_canvas_masks: { ...binding, field: 'mask' } };
  const first = withInputBinding(original, 'image', binding),
    second = withInputBinding(first, 'image', { ...binding, field: 'guides' });
  expect(readInputBinding(second, 'image', 'reference')).toEqual(binding);
  expect(readInputBinding(second, 'image', 'guides')?.field).toBe('guides');
  expect(second.sugarcubes_canvas_masks).toBe(original.sugarcubes_canvas_masks);
  expect(readInputBinding(second, 'mask')?.field).toBe('mask');
  expect(original).not.toHaveProperty('sugarcubes_canvas_images');
  expect(() =>
    readInputBinding(
      { sugarcubes_canvas_images: { version: 9, fields: {} } },
      'image',
      'reference',
    ),
  ).toThrow('Invalid image');
});
