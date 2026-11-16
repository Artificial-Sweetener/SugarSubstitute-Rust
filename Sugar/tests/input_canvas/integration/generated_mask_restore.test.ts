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
/** Prove a workflow loaded from a generated image gets back the masks it was queued with. */
import { jest } from '@jest/globals';
import { randomUUID } from 'node:crypto';
import { sourceMaskFixture } from '../support/source_mask_fixture.js';
import { createComfyInputBinding } from '../../../frontend/comfyui/ui/input_canvas/infrastructure/ComfyInputBinding.js';
import { withGeneratedSources } from '../../../frontend/comfyui/ui/input_canvas/infrastructure/GeneratedMaskSources.js';
import type { InputBinding } from '../../../frontend/comfyui/ui/input_canvas/domain/InputBinding.js';
import type { ComfyNode } from '../../../frontend/comfyui/ui/types/graph.js';

beforeAll(() =>
  Object.defineProperty(crypto, 'randomUUID', { configurable: true, value: randomUUID }),
);
afterEach(() => {
  document.body.replaceChildren();
  jest.restoreAllMocks();
});

/** A mask painted in the canvas's own document, as a queued workflow records it. */
function layer(layerId: string) {
  return {
    kind: 'canvas' as const,
    canvasId: 'editor',
    documentId: 'input-2',
    target: { kind: 'layer' as const, layerId },
  };
}

/** The ordered mask loader of a workflow embedded in a generated image. */
function generated(): ComfyNode {
  return {
    id: 5,
    type: 'SimpleSyrup.LoadMaskBatch',
    properties: {
      sugarcubes_canvas_masks: {
        version: 2,
        field: 'image',
        selected: 'second',
        files: [],
        entries: [
          { id: 'first', label: 'Mask 1', source: layer('layer-2') },
          { id: 'second', label: 'Mask 2', source: layer('layer-3') },
        ],
      },
    },
    widgets: [{ name: 'image', value: [] }],
    // Comfy keeps keys it does not know on the configured node.
    sugarcubes_widget_values: {
      image: ['sugarcubes/sources/one.png', 'sugarcubes/sources/two.png'],
      channel: 'red',
    },
  };
}

test('masks of a generated image return as the files they were queued with, once', async () => {
  const f = sourceMaskFixture();
  const loader = f.loaders.read('SimpleSyrup.LoadMaskBatch')!;
  const node = generated();
  const verifyFile = jest.fn(async (file: string) => file !== 'sugarcubes/sources/two.png');
  const binding = createComfyInputBinding(node, loader, {
    ...f.ports,
    canvases: { editable: () => true, provisions: () => true },
    verifyFile,
  });
  await binding.initialize();
  // The available capture replaces its canvas source; the missing one leaves its mask as it was.
  expect(binding.state.entries.map((entry) => entry.source)).toEqual([
    { kind: 'file', file: 'sugarcubes/sources/one.png' },
    layer('layer-3'),
  ]);
  expect(binding.state.selected).toBe('second');
  expect(node.sugarcubes_widget_values).toEqual({ channel: 'red' });
  // The canvas imports the file and the loader returns to its layer; nothing brings it back.
  binding.provision(new Map([['first', layer('layer-9')]]));
  await binding.initialize();
  expect(binding.state.entries.map((entry) => entry.source)).toEqual([
    layer('layer-9'),
    layer('layer-3'),
  ]);
  expect(verifyFile).toHaveBeenCalledTimes(2);
  expect(f.report).not.toHaveBeenCalled();
  binding.dispose();
});

test('the record waits for a canvas that provisions the workflow, and a live workflow has none', async () => {
  const f = sourceMaskFixture();
  const loader = f.loaders.read('SimpleSyrup.LoadMaskBatch')!;
  const node = generated();
  let provisions = false;
  const binding = createComfyInputBinding(node, loader, {
    ...f.ports,
    canvases: { editable: () => true, provisions: () => provisions },
  });
  await binding.initialize();
  expect(binding.state.entries.map((entry) => entry.source)).toEqual([
    layer('layer-2'),
    layer('layer-3'),
  ]);
  expect(node.sugarcubes_widget_values).toHaveProperty('image');
  provisions = true;
  await binding.initialize();
  expect(binding.state.entries.map((entry) => entry.source)).toEqual([
    { kind: 'file', file: 'sugarcubes/sources/one.png' },
    { kind: 'file', file: 'sugarcubes/sources/two.png' },
  ]);
  binding.dispose();

  const live = generated();
  delete live.sugarcubes_widget_values;
  const kept = createComfyInputBinding(live, loader, {
    ...f.ports,
    canvases: { editable: () => true, provisions: () => true },
  });
  await kept.initialize();
  expect(kept.state.entries.map((entry) => entry.source)).toEqual([
    layer('layer-2'),
    layer('layer-3'),
  ]);
  kept.dispose();
});

test('files pair with entries only while the record still matches the binding', () => {
  const binding: InputBinding = {
    version: 2,
    field: 'image',
    selected: 'second',
    files: [],
    entries: [
      { id: 'first', label: 'Mask 1', source: layer('layer-2') },
      { id: 'second', label: 'Mask 2', source: layer('layer-3') },
    ],
  };
  const all = (): boolean => true;
  // A scalar loader's one file belongs to its selected entry.
  expect(
    withGeneratedSources(binding, false, ['one.png'], [true], all)?.entries.map(
      (entry) => entry.source,
    ),
  ).toEqual([layer('layer-2'), { kind: 'file', file: 'one.png' }]);
  // A record of another length describes a different set of masks.
  expect(withGeneratedSources(binding, true, ['one.png'], [true], all)).toBeNull();
  expect(
    withGeneratedSources(binding, false, ['one.png', 'two.png'], [true, true], all),
  ).toBeNull();
  // Sources of a canvas that provisions nothing, and sources that are not layers, stay.
  expect(
    withGeneratedSources(binding, true, ['one.png', 'two.png'], [true, true], () => false),
  ).toBeNull();
  const selection: InputBinding = {
    ...binding,
    entries: binding.entries.map((entry) => ({
      ...entry,
      source: { ...layer('x'), target: { kind: 'selection' as const } },
    })),
  };
  expect(
    withGeneratedSources(selection, true, ['one.png', 'two.png'], [true, true], all),
  ).toBeNull();
});
