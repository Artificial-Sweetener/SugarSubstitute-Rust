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
/** Prove that provisioning binds mask loaders without overriding the user's explicit choices. */
import { jest } from '@jest/globals';
import { InputBindingController } from '../../../frontend/comfyui/ui/input_canvas/application/InputBindingController.js';
import type { InputBinding } from '../../../frontend/comfyui/ui/input_canvas/domain/InputBinding.js';
import type { InputLoader } from '../../../frontend/comfyui/ui/input_canvas/domain/InputLoader.js';
import { readInputBinding } from '../../../frontend/comfyui/ui/input_canvas/infrastructure/InputBindingMetadata.js';
import type { CanvasSourceHost } from '../../../integrations/contracts/CanvasSources.js';
import type {
  CanvasInputSource,
  CanvasSourceContext,
} from '../../../integrations/contracts/CanvasProtocol.generated.js';

const context: CanvasSourceContext = {
  kind: 'context',
  observesChanges: true,
  supported: true,
  document: { id: 'doc', geometryRevision: 'g', width: 4, height: 4 },
  revision: '1',
  targets: ['selection', 'layer', 'composition'],
  layers: [{ id: 'layer-1', name: 'Mask', parentId: null, depth: 0, visible: true }],
  currentLayerId: 'layer-1',
};
const layer: CanvasInputSource = {
  kind: 'canvas',
  canvasId: 'native',
  documentId: 'doc',
  target: { kind: 'layer', layerId: 'layer-1' },
};

/** Build a controller over in-memory persistence. */
function controller(loader: Partial<InputLoader> = {}, provisions = true) {
  let saved: InputBinding | null = null;
  const host: CanvasSourceHost = {
    canvasId: 'native',
    changes: null,
    execute: jest.fn(async () => context),
  };
  let ids = 0;
  const value = new InputBindingController(
    {
      kind: 'mask',
      classType: 'LoadImageMask',
      field: 'image',
      ordered: false,
      channel: 'channel',
      ...loader,
    },
    {
      read: () => saved,
      load: async () => saved,
      write: (state) => {
        saved = state;
      },
      files: () => [],
      settings: () => ({ kind: 'mask', classType: 'LoadImageMask', field: 'image', inputs: {} }),
      subscribe: () => () => {},
    },
    {
      version: 2,
      field: 'image',
      files: [],
      selected: 'entry-0',
      entries: [{ id: 'entry-0', label: 'Mask 1', source: { kind: 'file', file: 'mask.png' } }],
    },
    () => host,
    { interpret: jest.fn<never>(), load: jest.fn<never>() },
    () => 'entry-' + String(++ids),
    jest.fn(),
    { prepare: jest.fn<never>() },
    { editable: () => false, provisions: () => provisions },
    () => {},
  );
  return { value, saved: () => saved };
}

test('a provisioning canvas leaves unconfigured loaders on their files instead of defaulting', async () => {
  const provisioned = controller();
  await provisioned.value.initialize();
  expect(provisioned.saved()).toBeNull();
  const peer = controller({}, false);
  await peer.value.initialize();
  expect(peer.saved()?.entries[0]?.source).toMatchObject({ target: { kind: 'selection' } });
});

test('provisioning binds the scalar entry and an explicit choice blocks later provisioning', async () => {
  const f = controller();
  await f.value.initialize();
  f.value.provision(new Map([[null, layer]]));
  expect(f.saved()).toMatchObject({ entries: [{ id: 'entry-0', source: layer }] });
  expect(f.saved()?.explicit).toBeUndefined();
  f.value.changeSource({
    kind: 'canvas',
    canvasId: 'native',
    documentId: null,
    target: { kind: 'selection' },
  });
  expect(f.saved()?.explicit).toBe(true);
  f.value.provision(new Map([[null, layer]]));
  expect(f.saved()?.entries[0]?.source).toMatchObject({ target: { kind: 'selection' } });
  f.value.changeSource(layer);
  f.value.provision(new Map([[null, layer]]));
  expect(f.saved()?.explicit).toBeUndefined();
  expect(
    readInputBinding({ sugarcubes_canvas_masks: { ...f.saved(), explicit: true } }, 'mask')
      ?.explicit,
  ).toBe(true);
  expect(() =>
    readInputBinding({ sugarcubes_canvas_masks: { ...f.saved(), explicit: 'yes' } }, 'mask'),
  ).toThrow('Invalid canvas source binding.');
});

test('ordered loaders bind each region by entry and new regions wait for provisioning', async () => {
  const f = controller({ classType: 'SimpleSyrup.LoadMaskBatch', ordered: true });
  await f.value.initialize();
  f.value.add();
  const [first, second] = f.saved()?.entries ?? [];
  expect(second?.source).toEqual({ kind: 'file', file: null });
  const other = { ...layer, target: { kind: 'layer', layerId: 'layer-2' } } as const;
  f.value.provision(new Map([[second?.id ?? '', other]]));
  expect(f.saved()?.entries.map((entry) => entry.source)).toEqual([first?.source, other]);
});

test('imported regions append as provisioned file entries and select the last one', async () => {
  const f = controller({ classType: 'SimpleSyrup.LoadMaskBatch', ordered: true });
  await f.value.initialize();
  f.value.append(['sugarcubes/sources/a.png', 'sugarcubes/sources/b.png']);
  expect(f.saved()?.entries.map((entry) => [entry.label, entry.source])).toEqual([
    ['Mask 1', { kind: 'file', file: 'mask.png' }],
    ['Mask 2', { kind: 'file', file: 'sugarcubes/sources/a.png' }],
    ['Mask 3', { kind: 'file', file: 'sugarcubes/sources/b.png' }],
  ]);
  expect(f.saved()?.selected).toBe(f.saved()?.entries[2]?.id);
  expect(f.saved()?.explicit).toBeUndefined();
  expect(() => controller().value.append(['x.png'])).toThrow('one source');
});

test('a loaded mask file seeds the provisioned entry and hands its source back to provisioning', async () => {
  const f = controller();
  await f.value.initialize();
  expect(f.value.provisioned).toBe(true);
  f.value.provision(new Map([[null, layer]]));
  f.value.changeSource({
    kind: 'canvas',
    canvasId: 'native',
    documentId: null,
    target: { kind: 'selection' },
  });
  f.value.loadFile('sugarcubes/sources/picked.png');
  expect(f.saved()?.entries).toEqual([
    {
      id: 'entry-0',
      label: 'Mask 1',
      source: { kind: 'file', file: 'sugarcubes/sources/picked.png' },
    },
  ]);
  expect(f.saved()?.explicit).toBeUndefined();
  f.value.provision(new Map([[null, layer]]));
  expect(f.saved()?.entries[0]?.source).toEqual(layer);
  const peer = controller({}, false);
  await peer.value.initialize();
  expect(peer.value.provisioned).toBe(false);
  expect(() => peer.value.loadFile('picked.png')).toThrow('Only an input canvas');
});
