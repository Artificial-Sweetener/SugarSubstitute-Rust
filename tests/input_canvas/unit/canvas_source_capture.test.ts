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
/** Prove reusable target policy and coherent acquisition before mask-specific interpretation. */
import { jest } from '@jest/globals';
import { CanvasSourceCapture } from '../../../frontend/comfyui/ui/input_canvas/application/CanvasSourceCapture.js';
import {
  chooseCanvasInput,
  defaultCanvasInput,
  CANVAS_INPUT_CHOICES,
} from '../../../frontend/comfyui/ui/input_canvas/domain/CanvasInputSource.js';
import type { CanvasSourceHost } from '../../../integrations/contracts/CanvasSources.js';
import type {
  CanvasSourceContext,
  CanvasSourceQuery,
  CanvasSourceResult,
  CanvasInputSource,
} from '../../../integrations/contracts/CanvasProtocol.generated.js';
import { encodeCanvasRaster } from '../../../integrations/shared/CanvasRasterEncoding.js';

const document = { id: 'doc', geometryRevision: 'size', width: 2, height: 1 };
const context: CanvasSourceContext = {
  kind: 'context',
  supported: true,
  document,
  revision: '1',
  targets: ['selection', 'current-layer', 'layer', 'composition', 'composition-from-layer'],
  layers: [{ id: '2', name: '<Layer>', parentId: null, depth: 0, visible: true }],
  currentLayerId: '2',
};

/** Bind one deterministic native boundary while exercising the real application capture owner. */
function fixture() {
  const execute = jest.fn(
    async (query: CanvasSourceQuery): Promise<CanvasSourceResult> =>
      query.type === 'context'
        ? context
        : {
            kind: 'captured',
            snapshots: query.targets.map((target) => ({
              target,
              document,
              revision: '1',
              pixels: encodeCanvasRaster(2, 1, 'rgba8', new Uint8Array(8)),
            })),
          },
  );
  const host: CanvasSourceHost = { canvasId: 'editor', execute, subscribe: () => () => {} };
  return { execute, service: new CanvasSourceCapture((id) => (id === 'editor' ? host : null)) };
}

test('selection is the capable-canvas default while explicit layer identity stays document-bound', () => {
  expect(CANVAS_INPUT_CHOICES.map((choice) => choice.label)).toEqual([
    'Canvas Selection',
    'Current Layer',
    'Specific Layer',
    'Current Composition',
    'Current Comp From Layer',
    'Load From File',
  ]);
  expect(defaultCanvasInput('editor', context, 'original.png', 'selection')).toEqual({
    kind: 'canvas',
    canvasId: 'editor',
    documentId: null,
    target: { kind: 'selection' },
  });
  expect(defaultCanvasInput('web', null, 'original.png', 'selection')).toEqual({
    kind: 'file',
    file: 'original.png',
  });
  expect(
    chooseCanvasInput('editor', context, { kind: 'composition-from-layer', layerId: '2' }),
  ).toEqual({
    kind: 'canvas',
    canvasId: 'editor',
    documentId: 'doc',
    target: { kind: 'composition-from-layer', layerId: '2' },
  });
  expect(() => chooseCanvasInput('editor', context, { kind: 'layer', layerId: 'missing' })).toThrow(
    'available layer',
  );
});

test('deduplicates targets into a coherent capture and preserves caller order and identity', async () => {
  const f = fixture();
  const source: CanvasInputSource = {
    kind: 'canvas',
    canvasId: 'editor',
    documentId: null,
    target: { kind: 'current-layer' },
  };
  const result = await f.service.capture(
    [
      { key: 'first', source },
      { key: 'second', source },
    ],
    0,
  );
  expect([...result.keys()]).toEqual(['first', 'second']);
  expect(result.get('first')).toBe(result.get('second'));
  expect(f.execute).toHaveBeenLastCalledWith({
    type: 'capture',
    document,
    targets: [{ kind: 'current-layer' }],
    maximumSize: 0,
  });
  expect(f.execute).toHaveBeenCalledTimes(2);
});

test('rejects document switches and unavailable explicit layers before native capture', async () => {
  const f = fixture();
  await expect(
    f.service.capture(
      [
        {
          key: 'entry',
          source: {
            kind: 'canvas',
            canvasId: 'editor',
            documentId: 'other',
            target: { kind: 'layer', layerId: '2' },
          },
        },
      ],
      0,
    ),
  ).rejects.toThrow('Open the source document');
  expect(f.execute).toHaveBeenCalledTimes(1);
  await expect(
    f.service.capture(
      [
        {
          key: 'entry',
          source: {
            kind: 'canvas',
            canvasId: 'editor',
            documentId: 'doc',
            target: { kind: 'layer', layerId: 'missing' },
          },
        },
      ],
      0,
    ),
  ).rejects.toThrow('Choose a layer');
  expect(f.execute).toHaveBeenCalledTimes(2);
});

test('rejects incomplete or incoherent native results instead of substituting other pixels', async () => {
  const f = fixture();
  const source: CanvasInputSource = {
    kind: 'canvas',
    canvasId: 'editor',
    documentId: null,
    target: { kind: 'selection' },
  };
  f.execute
    .mockResolvedValueOnce(context)
    .mockResolvedValueOnce({ kind: 'captured', snapshots: [] });
  await expect(f.service.capture([{ key: 'one', source }], 288)).rejects.toThrow('incomplete');
  f.execute.mockResolvedValueOnce(context).mockResolvedValueOnce({
    kind: 'captured',
    snapshots: [
      {
        target: { kind: 'selection' },
        document: { ...document, id: 'other' },
        revision: '2',
        pixels: encodeCanvasRaster(2, 1, 'coverage8', new Uint8Array(2)),
      },
    ],
  });
  await expect(f.service.capture([{ key: 'one', source }], 288)).rejects.toThrow('identity');
});

test('accepts structurally identical adapter values regardless of JSON property ordering', async () => {
  const f = fixture(),
    source: CanvasInputSource = {
      kind: 'canvas',
      canvasId: 'editor',
      documentId: 'doc',
      target: { kind: 'layer', layerId: '2' },
    };
  f.execute.mockResolvedValueOnce(context).mockResolvedValueOnce({
    kind: 'captured',
    snapshots: [
      {
        target: { layerId: '2', kind: 'layer' },
        document: { height: 1, width: 2, geometryRevision: 'size', id: 'doc' },
        revision: '1',
        pixels: encodeCanvasRaster(2, 1, 'rgba8', new Uint8Array(8)),
      },
    ],
  });
  expect((await f.service.capture([{ key: 'one', source }], 0)).has('one')).toBe(true);
});

test('rejects cropped thumbnails that omit the full document frame', async () => {
  const f = fixture(),
    doc = { ...document, width: 100, height: 200 };
  f.execute.mockResolvedValueOnce({ ...context, document: doc }).mockResolvedValueOnce({
    kind: 'captured',
    snapshots: [
      {
        target: { kind: 'selection' },
        document: doc,
        revision: '1',
        pixels: encodeCanvasRaster(10, 10, 'coverage8', new Uint8Array(100)),
      },
    ],
  });
  await expect(
    f.service.capture(
      [
        {
          key: 'one',
          source: {
            kind: 'canvas',
            canvasId: 'editor',
            documentId: null,
            target: { kind: 'selection' },
          },
        },
      ],
      100,
    ),
  ).rejects.toThrow('dimensions');
});

test('rejects a collection that exceeds the aggregate raster budget before decoding it', async () => {
  const f = fixture(),
    doc = { ...document, width: 4096, height: 4096 };
  f.execute.mockResolvedValueOnce({ ...context, document: doc }).mockResolvedValueOnce({
    kind: 'captured',
    snapshots: [
      {
        target: { kind: 'current-layer' },
        document: doc,
        revision: '1',
        pixels: { width: 4096, height: 4096, format: 'rgba8', data: '' },
      },
      {
        target: { kind: 'composition' },
        document: doc,
        revision: '1',
        pixels: { width: 4096, height: 4096, format: 'rgba8', data: '' },
      },
    ],
  });
  await expect(
    f.service.capture(
      [
        {
          key: 'one',
          source: {
            kind: 'canvas',
            canvasId: 'editor',
            documentId: null,
            target: { kind: 'current-layer' },
          },
        },
        {
          key: 'two',
          source: {
            kind: 'canvas',
            canvasId: 'editor',
            documentId: null,
            target: { kind: 'composition' },
          },
        },
      ],
      0,
    ),
  ).rejects.toThrow('transfer budget');
});

test('rejects an image substituted for selection coverage at the adapter boundary', async () => {
  const f = fixture();
  await expect(
    f.service.capture(
      [
        {
          key: 'selection',
          source: {
            kind: 'canvas',
            canvasId: 'editor',
            documentId: null,
            target: { kind: 'selection' },
          },
        },
      ],
      288,
    ),
  ).rejects.toThrow('representation');
});
