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
/** Prove provisioning from live graphs through the real association, binding and state owners. */
import { jest } from '@jest/globals';
import { randomUUID } from 'node:crypto';
import type { ComfyNode } from '../../../frontend/comfyui/ui/types/graph.js';
import { ComfyInputAssociations } from '../../../frontend/comfyui/ui/input_canvas/infrastructure/ComfyInputAssociations.js';
import { ComfyInputBindings } from '../../../frontend/comfyui/ui/input_canvas/infrastructure/ComfyInputBindings.js';
import { ComfyInputLoaders } from '../../../frontend/comfyui/ui/input_canvas/infrastructure/ComfyInputLoaders.js';
import { ComfyWorkflowInputState } from '../../../frontend/comfyui/ui/input_canvas/infrastructure/ComfyWorkflowInputState.js';
import {
  ComfyWorkflowSurfaceGraph,
  MASK_OPACITY_KEY,
} from '../../../frontend/comfyui/ui/input_canvas/infrastructure/ComfyWorkflowSurfaceGraph.js';
import { WorkflowSurfaceProvisioning } from '../../../frontend/comfyui/ui/input_canvas/application/WorkflowSurfaceProvisioning.js';
import { MASK_BINDING_KEY } from '../../../frontend/comfyui/ui/input_canvas/domain/InputBinding.js';
import type { CanvasSourceHost } from '../../../integrations/contracts/CanvasSources.js';
import type { CanvasWorkflowHost } from '../../../integrations/contracts/CanvasWorkflowSurfaces.js';
import type {
  CanvasWorkflowCommand,
  CanvasWorkflowResult,
} from '../../../integrations/contracts/CanvasProtocol.generated.js';
import { associationGraph } from '../support/association_graph.js';

beforeAll(() =>
  Object.defineProperty(crypto, 'randomUUID', { configurable: true, value: randomUUID }),
);

/** Define the installed loaders, a latent root with size fields and spatial consumers. */
function loaders(): ComfyInputLoaders {
  const value = new ComfyInputLoaders();
  value.define({
    LoadImage: {
      input: { required: { image: ['COMBO', { image_upload: true }] } },
      output: ['IMAGE', 'MASK'],
    },
    LoadImageMask: {
      input: { required: { image: ['COMBO', { image_upload: true }], channel: ['COMBO'] } },
      output: ['MASK'],
    },
  });
  return value;
}

/** Build one live node with typed sockets and widgets. */
function node(
  id: number,
  type: string,
  inputs: readonly [string, string][],
  outputs: readonly string[],
  widgets: Record<string, unknown> = {},
): ComfyNode {
  return {
    id,
    type,
    title: type,
    properties: {},
    widgets: Object.entries(widgets).map(([name, value]) => ({ name, value })),
    inputs: inputs.map(([name, type]) => ({ name, type, link: null })),
    outputs: outputs.map((type) => ({ name: type, type, links: [] })),
  };
}

/** Compose provisioning with a canvas that numbers documents and layers deterministically. */
function fixture() {
  const native = associationGraph();
  const definitions = loaders();
  const source: CanvasSourceHost = {
    canvasId: 'native',
    changes: null,
    execute: jest.fn(async () => ({
      kind: 'context' as const,
      observesChanges: true,
      supported: true,
      document: null,
      revision: '1',
      targets: ['selection' as const, 'layer' as const, 'composition' as const],
      layers: [],
      currentLayerId: null,
    })),
  };
  const report = jest.fn();
  const bindings = new ComfyInputBindings(definitions, {
    host: () => source,
    canvases: { editable: () => false, provisions: () => true },
    refreshPreviews: () => {},
    files: { interpret: jest.fn<never>(), load: jest.fn<never>() },
    maskEditing: { prepare: jest.fn<never>() },
    legacy: { read: async () => null },
    verifyFile: async () => true,
    verifiedFile: () => true,
    report,
  });
  const associations = new ComfyInputAssociations(
    definitions,
    bindings,
    () => native.graph,
    report,
    { warn: jest.fn() },
  );
  const commands: CanvasWorkflowCommand[] = [];
  const workflow: CanvasWorkflowHost = {
    canvasId: 'native',
    execute: jest.fn(async (command: CanvasWorkflowCommand): Promise<CanvasWorkflowResult> => {
      commands.push(command);
      if (command.type !== 'sync') return { kind: 'applied' };
      return {
        kind: 'synced',
        surfaces: command.surfaces.map((surface, index) => ({
          key: surface.key,
          document: { id: 'doc-' + String(index), geometryRevision: 'g', width: 8, height: 8 },
          masks: surface.masks.map((mask, layer) => ({
            key: mask.key,
            layerId: `layer-${String(index)}-${String(layer)}`,
          })),
        })),
      };
    }),
  };
  let ids = 0;
  const state = new ComfyWorkflowInputState(
    () => native.graph,
    () => String(++ids),
  );
  const graph = new ComfyWorkflowSurfaceGraph(associations, bindings, definitions, state, report);
  const provisioning = new WorkflowSurfaceProvisioning(
    graph,
    () => workflow,
    () => '#479ef5',
    { read: async () => ({ width: 8, height: 8 }) },
    jest.fn(),
    report,
  );
  return { native, associations, bindings, graph, provisioning, commands, report, definitions };
}

/** Let coalesced microtasks and the fake canvas settle. */
async function settle(provisioning: WorkflowSurfaceProvisioning): Promise<void> {
  for (let round = 0; round < 4; round++) {
    await provisioning.request();
    await new Promise((resolve) => setTimeout(resolve, 0));
  }
}

test('an image loader, a paired mask loader and an inpaint consumer provision one surface', async () => {
  const f = fixture();
  const image = node(1, 'LoadImage', [], ['IMAGE', 'MASK'], { image: 'photo.png' });
  const mask = node(2, 'LoadImageMask', [], ['MASK'], { image: 'old-mask.png', channel: 'alpha' });
  const inpaint = node(
    3,
    'Inpaint',
    [
      ['pixels', 'IMAGE'],
      ['mask', 'MASK'],
    ],
    ['IMAGE'],
  );
  f.native.add(image, mask, inpaint);
  f.native.connect(1, image, 0, inpaint, 0);
  f.native.connect(2, mask, 0, inpaint, 1);
  f.associations.start();
  await settle(f.provisioning);
  const sync = f.commands.find((command) => command.type === 'sync');
  expect(sync).toMatchObject({
    surfaces: [
      {
        key: JSON.stringify(['1', 'image']),
        label: 'LoadImage',
        base: { kind: 'file', file: { filename: 'photo.png', subfolder: '', type: 'input' } },
        masks: [{ key: JSON.stringify(['2', 'image']), color: '#479ef5', opacity: 0.5 }],
      },
    ],
  });
  expect(f.native.graph.extra).toMatchObject({
    sugarcubes_input_canvas: { version: 1, id: 'workflow-1', route: 'output' },
  });
  const saved = mask.properties?.[MASK_BINDING_KEY] as { entries: { source: unknown }[] };
  expect(saved.entries.map((entry) => entry.source)).toEqual([
    {
      kind: 'canvas',
      canvasId: 'native',
      documentId: 'doc-0',
      target: { kind: 'layer', layerId: 'layer-0-0' },
    },
  ]);
  expect(image.widgets?.[0]?.value).toBe('photo.png');
  expect(f.report).not.toHaveBeenCalled();
});

test('a mask with no image is sized by the latent root it meets and follows size edits', async () => {
  const f = fixture();
  const mask = node(2, 'LoadImageMask', [], ['MASK'], { image: '', channel: 'alpha' });
  const latent = node(4, 'EmptyLatentImage', [], ['LATENT'], {
    width: 512,
    height: 768,
    batch_size: 1,
  });
  const noise = node(
    5,
    'SetLatentNoiseMask',
    [
      ['samples', 'LATENT'],
      ['mask', 'MASK'],
    ],
    ['LATENT'],
  );
  f.native.add(mask, latent, noise);
  f.native.connect(1, latent, 0, noise, 0);
  f.native.connect(2, mask, 0, noise, 1);
  f.associations.start();
  await settle(f.provisioning);
  expect(f.provisioning.state?.surfaces).toMatchObject([
    {
      image: null,
      base: { kind: 'blank', width: 512, height: 768 },
      authority: [{ node: '4', output: 0, width: 'width', height: 'height' }],
      masks: [{ key: JSON.stringify(['2', 'image']) }],
    },
  ]);
  const key = f.provisioning.state?.surfaces[0]?.key;
  latent.widgets![0]!.value = 1024;
  (latent.onWidgetChanged as (name: string) => void)('width');
  await settle(f.provisioning);
  expect(f.provisioning.state?.surfaces[0]).toMatchObject({
    key,
    base: { kind: 'blank', width: 1024, height: 768 },
  });
  f.graph.write(JSON.stringify(['2', 'image']), 0.2);
  expect(mask.properties?.[MASK_OPACITY_KEY]).toBe(0.2);
  await settle(f.provisioning);
  expect(f.provisioning.state?.surfaces[0]?.masks[0]?.opacity).toBe(0.2);
});

test('an explicit source choice is kept and loaders on multi-field nodes are skipped', async () => {
  const f = fixture();
  const image = node(1, 'LoadImage', [], ['IMAGE', 'MASK'], { image: 'photo.png' });
  const mask = node(2, 'LoadImageMask', [], ['MASK'], { image: '', channel: 'alpha' });
  const inpaint = node(
    3,
    'Inpaint',
    [
      ['pixels', 'IMAGE'],
      ['mask', 'MASK'],
    ],
    ['IMAGE'],
  );
  f.native.add(image, mask, inpaint);
  f.native.connect(1, image, 0, inpaint, 0);
  f.native.connect(2, mask, 0, inpaint, 1);
  const binding = f.bindings.get(mask, f.definitions.read('LoadImageMask')!);
  await binding.initialize();
  binding.changeSource({ kind: 'file', file: 'chosen.png' });
  f.associations.start();
  await settle(f.provisioning);
  expect(binding.state.explicit).toBe(true);
  expect(binding.state.entries[0]?.source).toEqual({ kind: 'file', file: 'chosen.png' });
  expect(f.provisioning.state?.surfaces[0]?.masks).toHaveLength(1);

  const twoFields = fixture();
  twoFields.definitions.define({
    DualLoader: {
      input: {
        required: {
          first: ['COMBO', { image_upload: true }],
          second: ['COMBO', { image_upload: true }],
        },
      },
      output: ['IMAGE'],
    },
  });
  const dual = node(7, 'DualLoader', [], ['IMAGE'], { first: 'a.png', second: 'b.png' });
  const consumer = node(8, 'Preview', [['images', 'IMAGE']], []);
  twoFields.native.add(dual, consumer);
  twoFields.native.connect(1, dual, 0, consumer, 0);
  twoFields.associations.start();
  await settle(twoFields.provisioning);
  expect(twoFields.provisioning.state?.surfaces).toEqual([]);
  expect(twoFields.native.graph.extra).toBeUndefined();
});
