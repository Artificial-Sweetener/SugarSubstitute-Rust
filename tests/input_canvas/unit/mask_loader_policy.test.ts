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
/** Prove mask-loader recognition, stable order and channel encoding independently of a native editor. */
import {
  recognizeInputLoaders,
  type InputLoaderDefinition,
} from '../../../frontend/comfyui/ui/input_canvas/domain/InputLoader.js';
import {
  moveInputEntry,
  type InputBinding,
} from '../../../frontend/comfyui/ui/input_canvas/domain/InputBinding.js';
import {
  interpretMaskSource,
  maskSourceImage,
} from '../../../frontend/comfyui/ui/input_canvas/domain/MaskSourcePixels.js';
import {
  encodeCanvasRaster,
  decodeCanvasRaster,
} from '../../../integrations/shared/CanvasRasterEncoding.js';
import { ComfyInputLoaders } from '../../../frontend/comfyui/ui/input_canvas/infrastructure/ComfyInputLoaders.js';
import { readInputBinding } from '../../../frontend/comfyui/ui/input_canvas/infrastructure/InputBindingMetadata.js';
const definition: InputLoaderDefinition = {
  classType: 'CustomMaskLoader',
  outputs: ['MASK'],
  fields: [{ name: 'file', upload: true, folder: 'input', ordered: false }],
  channel: 'channel',
};

test('recognizes one source field and distinguishes ordered loaders from downstream mask consumers', () => {
  expect(recognizeInputLoaders(definition)).toMatchObject([
    { kind: 'mask', field: 'file', ordered: false },
  ]);
  expect(
    recognizeInputLoaders({ ...definition, fields: [{ ...definition.fields[0]!, ordered: true }] }),
  ).toMatchObject([{ kind: 'mask', ordered: true }]);
  expect(recognizeInputLoaders({ ...definition, fields: [] })).toEqual([]);
  expect(recognizeInputLoaders({ ...definition, outputs: ['IMAGE', 'MASK'] })).toMatchObject([
    { kind: 'image', field: 'file' },
  ]);
  expect(
    recognizeInputLoaders({ ...definition, fields: [...definition.fields, ...definition.fields] }),
  ).toEqual([]);
  expect(
    recognizeInputLoaders({
      ...definition,
      fields: [{ ...definition.fields[0]!, folder: 'output' }],
    }),
  ).toEqual([]);
  expect(
    recognizeInputLoaders({ ...definition, classType: 'SimpleSyrup.LoadMaskBatch', fields: [] }),
  ).toMatchObject([{ kind: 'mask', field: 'image', ordered: true, channel: 'channel' }]);
});

test('decodes explicit batch metadata without guessing from class names', () => {
  const catalog = new ComfyInputLoaders();
  catalog.define({
    Other: {
      input: {
        required: {
          image: [['a'], { image_upload: true, allow_batch: true }],
          channel: [['red', 'alpha']],
        },
      },
      output: ['MASK'],
    },
  });
  expect(catalog.read('Other')).toMatchObject({ ordered: true, field: 'image' });
});

test('keeps region identity and selection during reorder', () => {
  const binding: InputBinding = {
    version: 2,
    field: 'image',
    selected: 'a',
    files: ['a.png', 'b.png'],
    entries: [
      { id: 'a', label: 'A', source: { kind: 'file', file: 'a.png' } },
      { id: 'b', label: 'B', source: { kind: 'file', file: 'b.png' } },
    ],
  };
  const moved = moveInputEntry(binding, 'a', 1);
  expect(moved.entries.map((entry) => entry.id)).toEqual(['b', 'a']);
  expect(moved.selected).toBe('a');
  expect(binding.entries[0]?.id).toBe('a');
  expect(moveInputEntry(binding, 'a', -1)).toBe(binding);
});

test.each(['alpha', 'red', 'green', 'blue'])(
  'preserves soft coverage through the selected %s channel',
  (channel) => {
    const source = Uint8Array.from([0, 1, 127, 128, 254, 255]);
    const image = maskSourceImage(encodeCanvasRaster(6, 1, 'coverage8', source));
    const decoded = decodeCanvasRaster(interpretMaskSource(image, channel));
    expect(decoded).toEqual(source);
  },
);

test('rejects corrupt persisted mask identities instead of silently dropping or rebinding entries', () => {
  expect(readInputBinding({}, 'mask')).toBeNull();
  expect(() =>
    readInputBinding(
      {
        sugarcubes_canvas_masks: { version: 2, field: 'image', selected: 'missing', entries: [] },
      },
      'mask',
    ),
  ).toThrow();
});
