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
/** Prove images and masks share capture, independent-field state and both execution preparation paths. */
import { nativeInputFields, nativeSourceField } from '../support/native_input_fields.js';
import { jest } from '@jest/globals';
import { randomUUID } from 'node:crypto';
import { sourceMaskFixture, sourceRgba, sourceCoverage } from '../support/source_mask_fixture.js';
import { decodeCanvasRaster } from '../../../integrations/shared/CanvasRasterEncoding.js';
import { ComfyInputBindings } from '../../../frontend/comfyui/ui/input_canvas/infrastructure/ComfyInputBindings.js';
import { createComfyInputBinding } from '../../../frontend/comfyui/ui/input_canvas/infrastructure/ComfyInputBinding.js';
import { readInputBinding } from '../../../frontend/comfyui/ui/input_canvas/infrastructure/InputBindingMetadata.js';
import { MasonryInputEditor } from '../../../frontend/comfyui/ui/input_canvas/presentation/MasonryInputEditor.js';
import { CubeDirectExecutionService } from '../../../frontend/comfyui/ui/cube/execution/CubeDirectExecutionService.js';
import type { CubeExecutionApiClient } from '../../../frontend/comfyui/ui/cube/execution/CubeExecutionApiClient.js';
import type { ComfyNode, ComfyGraph } from '../../../frontend/comfyui/ui/types/graph.js';

beforeAll(() =>
  Object.defineProperty(crypto, 'randomUUID', { configurable: true, value: randomUUID }),
);
afterEach(() => {
  document.body.replaceChildren();
  jest.restoreAllMocks();
});
/** Settle coalesced external host reads while retaining actual capture and binding owners. */
async function settled(): Promise<void> {
  for (let i = 0; i < 100; i++) await Promise.resolve();
}

test('captures image selections and mask selections coherently with distinct consumer representations', async () => {
  const f = sourceMaskFixture();
  const source = {
    kind: 'canvas' as const,
    canvasId: 'editor',
    documentId: null,
    target: { kind: 'selection' as const },
  };
  const captured = await f.capture.capture(
    [
      { key: 'mask', source },
      { key: 'image', source, representation: 'image' },
    ],
    0,
  );
  expect(captured.get('mask')?.pixels).toEqual(sourceCoverage);
  expect(captured.get('image')?.pixels.format).toBe('rgba8');
  expect([...decodeCanvasRaster(captured.get('image')!.pixels)].slice(4, 8)).toEqual([
    40, 50, 60, 127,
  ]);
  expect(f.execute.mock.calls.filter(([query]) => query.type === 'capture')).toHaveLength(1);
  expect(f.execute).toHaveBeenLastCalledWith(
    expect.objectContaining({ targets: [{ kind: 'selection' }, { kind: 'composition' }] }),
  );
  f.previews.dispose();
});

test('defaults an image loader to composition, displays actual image colors and preserves native mixed outputs', async () => {
  const f = sourceMaskFixture();
  f.loaders.define({ LoadImage: {} });
  const loader = f.loaders.read('LoadImage')!;
  const node: ComfyNode = {
    id: 7,
    type: 'LoadImage',
    properties: {},
    widgets: [
      { name: 'image', value: 'unrelated.png' },
      { name: 'upload', type: 'button' },
    ],
  };
  const binding = createComfyInputBinding(node, loader, f.ports);
  await binding.initialize();
  expect(binding.state.entries[0]?.source).toMatchObject({ target: { kind: 'composition' } });
  expect(node.widgets?.[0]?.value).toBe('');
  const paint = jest.fn();
  jest.spyOn(HTMLCanvasElement.prototype, 'getContext').mockReturnValue({
    createImageData: (width: number, height: number) => ({
      data: new Uint8ClampedArray(width * height * 4),
    }),
    putImageData: paint,
  } as unknown as CanvasRenderingContext2D);
  const view = new MasonryInputEditor(
    document,
    binding,
    f.previews,
    async () => 'actual.png',
    f.report,
    nativeInputFields(node),
    document.createElement('article'),
    null,
  );
  document.body.append(view.element);
  await settled();
  expect(nativeSourceField(node).value).toBe('composition');
  expect(paint).toHaveBeenLastCalledWith(
    expect.objectContaining({ data: Uint8ClampedArray.from(decodeCanvasRaster(sourceRgba)) }),
    0,
    0,
  );
  const payload = {
    workflow: {
      nodes: [
        {
          id: 7,
          type: 'LoadImage',
          properties: node.properties,
          sugarcubes_widget_values: { image: '' },
        },
      ],
    },
    output: {
      '7': { class_type: 'LoadImage', inputs: { image: '' } },
      '8': { class_type: 'MaskConsumer', inputs: { mask: ['7', 1] } },
    },
  };
  const prepared = await f.preparation.prepare(payload);
  expect(prepared).toMatchObject({
    output: {
      '7': { class_type: 'LoadImage', inputs: { image: 'immutable-1.png' } },
      '8': payload.output['8'],
    },
  });
  expect(payload.output['7'].inputs.image).toBe('');
  expect(f.report).not.toHaveBeenCalled();
  view.dispose();
  binding.dispose();
  f.previews.dispose();
});

test('owns multiple upload fields on a node with one native callback lifetime and one coherent queue snapshot', async () => {
  const f = sourceMaskFixture();
  f.loaders.define({
    Multi: {
      output: ['IMAGE'],
      input: {
        required: { reference: [[], { image_upload: true }], guide: [[], { image_upload: true }] },
      },
    },
  });
  const original = jest.fn(),
    removed = jest.fn();
  const node: ComfyNode = {
    id: 3,
    type: 'Multi',
    properties: {},
    onWidgetChanged: original,
    onRemoved: removed,
    widgets: [
      { name: 'reference', value: 'old.png' },
      { name: 'guide', value: 'old.png' },
    ],
  };
  const registry = new ComfyInputBindings(f.loaders, f.ports);
  registry.loaded(node);
  await registry.ready();
  const fields = f.loaders.all('Multi');
  const reference = registry.get(node, fields[0]!),
    guide = registry.get(node, fields[1]!);
  expect(reference).not.toBe(guide);
  expect(readInputBinding(node.properties, 'image', 'reference')).not.toBeNull();
  guide.changeSource({
    kind: 'canvas',
    canvasId: 'editor',
    documentId: null,
    target: { kind: 'current-layer' },
  });
  const prepared = await f.preparation.prepare({
    workflow: {
      nodes: [
        {
          id: 3,
          type: 'Multi',
          properties: node.properties,
          sugarcubes_widget_values: { reference: '', guide: '' },
        },
      ],
    },
    output: { '3': { class_type: 'Multi', inputs: { reference: '', guide: '', strength: 0.7 } } },
  });
  expect(prepared).toMatchObject({
    workflow: {
      nodes: [
        { sugarcubes_widget_values: { reference: 'immutable-1.png', guide: 'immutable-2.png' } },
      ],
    },
    output: {
      '3': { inputs: { reference: 'immutable-1.png', guide: 'immutable-2.png', strength: 0.7 } },
    },
  });
  expect(f.stage.mock.calls.map(([pixels]) => pixels)).toEqual([sourceRgba, sourceRgba]);
  registry.clear();
  expect(node.onWidgetChanged).toBe(original);
  expect(node.onRemoved).toBe(removed);
  expect(f.report).not.toHaveBeenCalled();
  f.previews.dispose();
});

test('preserves ordered image sources through Cube execution using the same preparation owner', async () => {
  const f = sourceMaskFixture();
  f.loaders.define({ 'SimpleSyrup.LoadImageList': {} });
  const loader = f.loaders.read('SimpleSyrup.LoadImageList')!;
  const node: ComfyNode = {
    id: 1,
    type: loader.classType,
    properties: {},
    widgets: [{ name: 'image', value: [] }],
  };
  const binding = createComfyInputBinding(node, loader, f.ports);
  await binding.initialize();
  binding.add();
  binding.changeSource({ kind: 'file', file: 'real-file.png' });
  binding.move(-1);
  const workflow = {
    nodes: [
      {
        id: 1,
        type: loader.classType,
        properties: node.properties,
        sugarcubes_widget_values: { image: [] },
      },
    ],
  };
  const queue = jest.fn(async () => ({ accepted: true }));
  const service = new CubeDirectExecutionService({
    client: { queue } as unknown as CubeExecutionApiClient,
    getGraph: () => ({ _nodes: [], serialize: () => workflow }) as ComfyGraph,
    getProximityMatches: () => [],
    prepareInputs: (payload) => f.preparation.prepare(payload),
  });
  await service.queue(0, {});
  expect(queue).toHaveBeenCalledWith(
    expect.objectContaining({
      workflow: expect.objectContaining({
        nodes: [
          expect.objectContaining({
            sugarcubes_widget_values: { image: ['real-file.png', 'immutable-1.png'] },
          }),
        ],
      }),
    }),
  );
  binding.dispose();
  f.previews.dispose();
});

test('preserves connected file inputs without silently replacing a graph-owned source', async () => {
  const f = sourceMaskFixture();
  f.loaders.define({ LoadImage: {} });
  const payload = {
    workflow: {
      nodes: [
        {
          id: 4,
          type: 'LoadImage',
          properties: {},
          sugarcubes_widget_values: { image: 'old.png' },
        },
      ],
    },
    output: { '4': { class_type: 'LoadImage', inputs: { image: ['other', 0] } } },
  };
  expect(await f.preparation.prepare(payload)).toBe(payload);
  expect(f.stage).not.toHaveBeenCalled();
  f.previews.dispose();
});
