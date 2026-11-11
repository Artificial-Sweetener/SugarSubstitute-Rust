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
/** Prove regional mask rows show their separator names, region colours and linked hover. */
import { jest } from '@jest/globals';
import { randomUUID } from 'node:crypto';
import { sourceMaskFixture } from '../support/source_mask_fixture.js';
import { nativeInputFields } from '../support/native_input_fields.js';
import { createComfyInputBinding } from '../../../frontend/comfyui/ui/input_canvas/infrastructure/ComfyInputBinding.js';
import { MasonryInputEditor } from '../../../frontend/comfyui/ui/input_canvas/presentation/MasonryInputEditor.js';
import type { InputSurfaceCardPort } from '../../../frontend/comfyui/ui/input_canvas/application/InputEditorPorts.js';
import type { RegionPresentation } from '../../../frontend/comfyui/ui/input_canvas/application/WorkflowRegionalPrompts.js';
import type { ComfyNode } from '../../../frontend/comfyui/ui/types/graph.js';

beforeAll(() =>
  Object.defineProperty(crypto, 'randomUUID', { configurable: true, value: randomUUID }),
);
beforeEach(() => {
  // jsdom supplies no native raster context; previews are outside this proof.
  jest.spyOn(HTMLCanvasElement.prototype, 'getContext').mockReturnValue({
    createImageData: (width: number, height: number) => ({
      data: new Uint8ClampedArray(width * height * 4),
    }),
    putImageData: jest.fn(),
  } as unknown as CanvasRenderingContext2D);
});
afterEach(() => {
  document.body.replaceChildren();
  jest.restoreAllMocks();
});

const loader = {
  kind: 'mask' as const,
  classType: 'SimpleSyrup.LoadMaskBatch',
  field: 'image',
  channel: 'channel',
  ordered: true,
};

/** Wait for committed metadata and preview microtasks. */
async function settled(): Promise<void> {
  for (let n = 0; n < 50; n++) await Promise.resolve();
}

/** A card port whose regional names and linked hover the test changes like the owners do. */
function card() {
  let regions = new Map<string, RegionPresentation>();
  let hovered: string | null = null;
  const surfaceListeners = new Set<() => void>(),
    hoverListeners = new Set<() => void>();
  const port: InputSurfaceCardPort = {
    activate: jest.fn(),
    openChosen: jest.fn(),
    hover: jest.fn(),
    hovered: () => hovered,
    subscribeHover: (listener) => {
      hoverListeners.add(listener);
      return () => hoverListeners.delete(listener);
    },
    region: (entry) => regions.get(entry) ?? null,
    opacity: () => null,
    changeOpacity: jest.fn(),
    canImport: () => false,
    importMasks: jest.fn(async () => {}),
    subscribe: (listener) => {
      surfaceListeners.add(listener);
      return () => surfaceListeners.delete(listener);
    },
  };
  return {
    port,
    name(next: Map<string, RegionPresentation>) {
      regions = next;
      for (const listener of surfaceListeners) listener();
    },
    link(entry: string | null) {
      hovered = entry;
      for (const listener of hoverListeners) listener();
    },
    listeners: () => surfaceListeners.size + hoverListeners.size,
  };
}

test('rows name, colour and link their regions and report their own hover', async () => {
  const f = sourceMaskFixture();
  const node: ComfyNode = {
    id: 4,
    type: loader.classType,
    widgets: [
      { name: 'image', value: [] },
      { name: 'channel', value: 'red' },
      { name: 'upload', type: 'button' },
    ],
    properties: {},
    graph: { beforeChange: jest.fn(), afterChange: jest.fn() },
  };
  const binding = createComfyInputBinding(node, loader, f.ports);
  await binding.initialize();
  binding.add();
  const [first, second] = binding.state.entries;
  if (!first || !second) throw new Error('Two regions are required.');
  const surface = card();
  const view = new MasonryInputEditor(
    document,
    binding,
    f.previews,
    async () => 'chosen.png',
    f.report,
    nativeInputFields(node),
    document.createElement('article'),
    null,
    false,
    surface.port,
  );
  document.body.append(view.element);
  await settled();
  const row = (id: string) =>
    view.element.querySelector<HTMLElement>(`[data-input-entry="${id}"]`) ?? undefined;
  const label = (id: string) => row(id)?.querySelector('.sc-input-editor__label')?.textContent;
  expect([label(first.id), label(second.id)]).toEqual([first.label, second.label]);
  surface.name(
    new Map([
      [first.id, { label: '<b>sky</b>', color: '#4f6df5' }],
      [second.id, { label: null, color: '#f5d235' }],
    ]),
  );
  expect([label(first.id), label(second.id)]).toEqual(['<b>sky</b>', second.label]);
  expect(view.element.querySelector('b')).toBeNull();
  expect(row(first.id)?.style.getPropertyValue('--sc-mask-color')).toBe('#4f6df5');
  expect(row(second.id)?.style.getPropertyValue('--sc-mask-color')).toBe('#f5d235');
  surface.link(second.id);
  expect([row(first.id)?.dataset.linkedHover, row(second.id)?.dataset.linkedHover]).toEqual([
    'false',
    'true',
  ]);
  surface.link(null);
  expect(row(second.id)?.dataset.linkedHover).toBe('false');
  row(first.id)?.dispatchEvent(new Event('pointerenter'));
  row(first.id)?.dispatchEvent(new Event('pointerleave'));
  expect(jest.mocked(surface.port.hover).mock.calls).toEqual([[first.id], [null]]);
  view.dispose();
  expect(surface.listeners()).toBe(0);
  binding.dispose();
  f.previews.dispose();
  expect(f.report).not.toHaveBeenCalled();
});
