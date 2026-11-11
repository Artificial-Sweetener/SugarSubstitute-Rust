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
/** Prove generic canvas sources feed ordinary and Cube execution with immutable, ordered mask inputs. */
import { jest } from '@jest/globals';
import { sourceMaskFixture, sourceDocument, sourceRgba } from '../support/source_mask_fixture.js';
import { CanvasSourceCapture } from '../../../frontend/comfyui/ui/input_canvas/application/CanvasSourceCapture.js';
import { CubeDirectExecutionService } from '../../../frontend/comfyui/ui/cube/execution/CubeDirectExecutionService.js';
import type { CubeExecutionApiClient } from '../../../frontend/comfyui/ui/cube/execution/CubeExecutionApiClient.js';
import type { ComfyGraph } from '../../../frontend/comfyui/ui/types/graph.js';
import type { InputBinding } from '../../../frontend/comfyui/ui/input_canvas/domain/InputBinding.js';
import type { CanvasInputSource } from '../../../integrations/contracts/CanvasProtocol.generated.js';

/** Keep live selection and explicit document-bound layers distinct in serialized workflows. */
function source(id: string): Exclude<CanvasInputSource, { kind: 'file' }> {
  return {
    kind: 'canvas',
    canvasId: 'editor',
    documentId: id === 'a' ? null : 'doc',
    target: id === 'a' ? { kind: 'selection' } : { kind: 'layer', layerId: id },
  };
}
/** Build an authored ordered source collection without retaining native mask-editing references. */
function workflow() {
  const binding: InputBinding = {
    version: 2,
    field: 'image',
    files: ['authored.png'],
    selected: 'b',
    entries: [
      { id: 'b', label: 'Second', source: source('b') },
      { id: 'file', label: 'File', source: { kind: 'file', file: 'authored.png' } },
      { id: 'a', label: 'First', source: source('a') },
    ],
  };
  return {
    nodes: [
      {
        id: 5,
        type: 'SimpleSyrup.LoadMaskBatch',
        properties: { sugarcubes_canvas_masks: binding },
        sugarcubes_widget_values: { image: ['authored.png'], channel: 'alpha' },
        widgets_values: [['authored.png'], 'alpha'],
      },
    ],
    links: [[1, 5, 0, 6, 0, 'MASK']],
  };
}

test('captures ordered sources together, preserves original source channels and substitutes immutable filenames', async () => {
  const f = sourceMaskFixture(),
    authored = workflow(),
    before = JSON.stringify(authored);
  const payload = {
    workflow: authored,
    output: {
      '5': {
        class_type: 'SimpleSyrup.LoadMaskBatch',
        inputs: { image: ['authored.png'], channel: 'alpha' },
      },
      '6': { class_type: 'Consumer', inputs: { mask: ['5', 0] } },
    },
  };
  expect((await f.preparation.prepare(payload)).payload).toMatchObject({
    output: {
      '5': {
        inputs: {
          image: { __value__: ['immutable-1.png', 'authored.png', 'immutable-2.png'] },
          channel: 'alpha',
        },
      },
      '6': payload.output['6'],
    },
  });
  expect(JSON.stringify(authored)).toBe(before);
  expect(f.execute).toHaveBeenLastCalledWith({
    type: 'capture',
    document: sourceDocument,
    targets: [{ kind: 'layer', layerId: 'b' }, { kind: 'selection' }],
    maximumSize: 0,
  });
  expect(f.stage).toHaveBeenCalledTimes(2);
  expect(f.stage.mock.calls[0]?.[0]).toEqual(sourceRgba);
  expect(f.stage.mock.calls[1]?.[0].format).toBe('rgba8');
});

test('rejects incomplete captures before upload and preserves the existing channel for a scalar loader', async () => {
  const f = sourceMaskFixture(),
    context = await f.execute({ type: 'context' });
  f.execute
    .mockResolvedValueOnce(context)
    .mockResolvedValueOnce({ kind: 'captured', snapshots: [] });
  await expect(f.preparation.prepare({ workflow: workflow() })).rejects.toThrow('incomplete');
  expect(f.stage).not.toHaveBeenCalled();
  const authored = workflow(),
    node = authored.nodes[0]!;
  node.type = 'LoadImageMask';
  node.properties.sugarcubes_canvas_masks = {
    ...node.properties.sugarcubes_canvas_masks,
    entries: node.properties.sugarcubes_canvas_masks.entries.slice(0, 1),
  };
  expect(
    (
      await f.preparation.prepare({
        workflow: authored,
        output: {
          '5': { class_type: 'LoadImageMask', inputs: { image: 'old.png', channel: 'blue' } },
        },
      })
    ).payload,
  ).toMatchObject({ output: { '5': { inputs: { image: 'immutable-1.png', channel: 'blue' } } } });
});

test('refuses disconnected sources and preserves workflows without applicable inputs', async () => {
  const f = sourceMaskFixture(),
    capture = new CanvasSourceCapture(() => null);
  await expect(capture.capture([{ key: 'a', source: source('a') }], 0)).rejects.toThrow(
    'unavailable',
  );
  const payload = { output: {} };
  expect((await f.preparation.prepare(payload)).payload).toBe(payload);
});

test('preserves file order without native reads and refuses to overwrite graph-connected file inputs', async () => {
  const f = sourceMaskFixture(),
    authored = workflow(),
    binding = authored.nodes[0]!.properties.sugarcubes_canvas_masks;
  authored.nodes[0]!.properties.sugarcubes_canvas_masks = {
    ...binding,
    entries: binding.entries.map((entry, index) => ({
      ...entry,
      source: { kind: 'file', file: `file-${String(index)}.png` },
    })),
  };
  expect(
    (
      await f.preparation.prepare({
        workflow: authored,
        output: {
          '5': {
            class_type: 'SimpleSyrup.LoadMaskBatch',
            inputs: { image: { __value__: ['original.png'] }, channel: 'red' },
          },
        },
      })
    ).payload,
  ).toMatchObject({
    output: {
      '5': { inputs: { image: { __value__: ['file-0.png', 'file-1.png', 'file-2.png'] } } },
    },
  });
  expect(f.execute).not.toHaveBeenCalled();
  expect(f.stage).not.toHaveBeenCalled();
  await expect(
    f.preparation.prepare({
      workflow: authored,
      output: {
        '5': {
          class_type: 'SimpleSyrup.LoadMaskBatch',
          inputs: { image: ['other-node', 0], channel: 'red' },
        },
      },
    }),
  ).rejects.toThrow('connected');
});

test('honors explicit native file edits over an older persisted source choice', async () => {
  const f = sourceMaskFixture(),
    authored = workflow();
  authored.nodes[0]!.sugarcubes_widget_values.image = ['new-file.png'];
  const payload = {
    workflow: authored,
    output: {
      '5': {
        class_type: 'SimpleSyrup.LoadMaskBatch',
        inputs: { image: { __value__: ['new-file.png'] }, channel: 'alpha' },
      },
    },
  };
  expect((await f.preparation.prepare(payload)).payload).toBe(payload);
  expect(f.execute).not.toHaveBeenCalled();
  expect(f.stage).not.toHaveBeenCalled();
});

test('uses the same source capture owner after native serialization in Cube direct execution', async () => {
  const f = sourceMaskFixture(),
    queue = jest.fn(async () => ({ accepted: true }));
  const graph = { _nodes: [], serialize: () => workflow() } as ComfyGraph;
  // The client is the external queue boundary; native serialization and preparation remain real.
  const service = new CubeDirectExecutionService({
    client: { queue } as unknown as CubeExecutionApiClient,
    getGraph: () => graph,
    getProximityMatches: () => [],
    prepareInputs: (payload) => f.preparation.prepare(payload),
  });
  await service.queue(0, {});
  expect(queue).toHaveBeenCalledWith(
    expect.objectContaining({
      workflow: expect.objectContaining({
        nodes: [
          expect.objectContaining({
            sugarcubes_widget_values: {
              image: ['immutable-1.png', 'authored.png', 'immutable-2.png'],
              channel: 'alpha',
            },
          }),
        ],
      }),
    }),
  );
  expect(f.execute.mock.calls.filter(([query]) => query.type === 'capture')).toHaveLength(1);
});

test('defaults an unconfigured capable loader even when no masonry card has mounted', async () => {
  const f = sourceMaskFixture();
  const payload = {
    workflow: {
      nodes: [
        {
          id: 9,
          type: 'LoadImageMask',
          properties: {},
          sugarcubes_widget_values: { image: 'unrelated.png', channel: 'red' },
        },
      ],
    },
    output: {
      '9': { class_type: 'LoadImageMask', inputs: { image: 'unrelated.png', channel: 'red' } },
    },
  };
  expect((await f.preparation.prepare(payload)).payload).toMatchObject({
    output: { '9': { inputs: { image: 'immutable-1.png', channel: 'red' } } },
    workflow: {
      nodes: [
        {
          properties: {
            sugarcubes_canvas_masks: {
              version: 2,
              entries: [{ source: { kind: 'canvas', target: { kind: 'selection' } } }],
            },
          },
        },
      ],
    },
  });
});

test('projects root and nested instances by compiled path while deduplicating the same source capture', async () => {
  const f = sourceMaskFixture(),
    authored = workflow();
  const payload = {
    workflow: {
      ...authored,
      nodes: [...authored.nodes, { id: 7, type: 'nested' }],
      definitions: { subgraphs: [{ id: 'nested', nodes: workflow().nodes }] },
    },
    output: {
      '5': { class_type: 'SimpleSyrup.LoadMaskBatch', inputs: { image: [], channel: 'red' } },
      '7:5': { class_type: 'SimpleSyrup.LoadMaskBatch', inputs: { image: [], channel: 'blue' } },
    },
  };
  expect((await f.preparation.prepare(payload)).payload).toMatchObject({
    output: {
      '5': {
        inputs: {
          image: { __value__: ['immutable-1.png', 'authored.png', 'immutable-2.png'] },
          channel: 'red',
        },
      },
      '7:5': {
        inputs: {
          image: { __value__: ['immutable-1.png', 'authored.png', 'immutable-2.png'] },
          channel: 'blue',
        },
      },
    },
  });
  expect(f.stage).toHaveBeenCalledTimes(2);
  expect(f.execute.mock.calls.filter(([query]) => query.type === 'capture')).toHaveLength(1);
});

test('keeps custom interpretation settings and connected channel resolution with the actual native loader', async () => {
  const f = sourceMaskFixture();
  f.loaders.define({
    CustomMask: {
      output: ['MASK'],
      input: {
        required: {
          image: [[], { image_upload: true, allow_batch: true }],
          channel: [['luminance', 'inverse']],
          invert: ['BOOLEAN'],
        },
      },
    },
  });
  const authored = workflow();
  authored.nodes[0]!.type = 'CustomMask';
  const output = {
    '5': {
      class_type: 'CustomMask',
      inputs: { image: 'authored.png', channel: ['settings', 0], invert: true },
    },
  };
  const prepared = (await f.preparation.prepare({ workflow: authored, output })).payload;
  expect(prepared).toMatchObject({
    output: {
      '5': {
        inputs: {
          image: { __value__: ['immutable-1.png', 'authored.png', 'immutable-2.png'] },
          channel: ['settings', 0],
          invert: true,
        },
      },
    },
  });
  expect(output['5'].inputs.image).toBe('authored.png');
});
