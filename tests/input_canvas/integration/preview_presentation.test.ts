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
/** Prove passive plugin previews, persistent replacement geometry and inline source availability. */
import { jest } from '@jest/globals';
import { randomUUID } from 'node:crypto';
import {
  sourceMaskFixture,
  sourceDocument,
  sourceCoverage,
} from '../support/source_mask_fixture.js';
import { nativeInputFields } from '../support/native_input_fields.js';
import { maskPreviewAssociation } from '../support/association_graph.js';
import { nativePresentationWidgets } from '../../../frontend/comfyui/ui/surface/NativePresentationWidgets.js';
import { createComfyInputBinding } from '../../../frontend/comfyui/ui/input_canvas/infrastructure/ComfyInputBinding.js';
import { MasonryInputEditor } from '../../../frontend/comfyui/ui/input_canvas/presentation/MasonryInputEditor.js';
import { InputLoaderPresentation } from '../../../frontend/comfyui/ui/input_canvas/infrastructure/InputLoaderPresentation.js';
import { resolveCubeFaceCardPresentation } from '../../../frontend/comfyui/ui/surface/CubeFaceCardPolicy.js';
import { createDefaultCubeSurfaceState } from '../../../frontend/comfyui/ui/surface/CubeSurfaceState.js';
import { publishPluginMode } from '../../../frontend/comfyui/ui/core/PluginMode.js';
import type { ComfyNode } from '../../../frontend/comfyui/ui/types/graph.js';
import type {
  CanvasSourceContext,
  CanvasPixelRaster,
} from '../../../integrations/contracts/CanvasProtocol.generated.js';

beforeAll(() =>
  Object.defineProperty(crypto, 'randomUUID', { configurable: true, value: randomUUID }),
);
beforeEach(() => {
  publishPluginMode(document, true);
  jest.spyOn(HTMLCanvasElement.prototype, 'getContext').mockReturnValue({
    createImageData: (width: number, height: number) => ({
      data: new Uint8ClampedArray(width * height * 4),
    }),
    putImageData: jest.fn(),
  } as unknown as CanvasRenderingContext2D);
});
afterEach(() => {
  publishPluginMode(document, false);
  document.body.replaceChildren();
  jest.restoreAllMocks();
});

/** Settle metadata and coalesced capture without depending on native render timers. */
async function settled(): Promise<void> {
  for (let index = 0; index < 100; index++) await Promise.resolve();
}

/** Keep source acquisition and interpretation real while controlling only the external canvas and file boundary. */
async function fixture(ordered = false, custom = false) {
  const f = sourceMaskFixture(),
    original = f.execute.getMockImplementation();
  if (!original) throw new Error('Missing canvas fixture.');
  const initial = await original({ type: 'context' });
  if (initial.kind !== 'context') throw new Error('Missing canvas context.');
  let context: CanvasSourceContext = initial;
  f.execute.mockImplementation((query) =>
    query.type === 'context' ? Promise.resolve(context) : original(query),
  );
  const node: ComfyNode = {
    id: 4,
    type: 'LoadImageMask',
    widgets: [
      { name: 'image', value: '' },
      { name: 'channel', value: 'red' },
    ],
    properties: {},
    graph: { beforeChange: jest.fn(), afterChange: jest.fn() },
  };
  const loader = {
    kind: 'mask' as const,
    classType: custom ? 'CustomMask' : 'LoadImageMask',
    field: 'image',
    channel: 'channel',
    ordered,
  };
  const binding = createComfyInputBinding(node, loader, f.ports);
  await binding.initialize();
  const association = await maskPreviewAssociation(f, node, loader);
  const view = new MasonryInputEditor(
    document,
    binding,
    f.previews,
    async () => 'chosen.png',
    f.report,
    nativeInputFields(node),
    document.createElement('article'),
    association.source,
  );
  document.body.append(view.element);
  await settled();
  return {
    ...f,
    node,
    binding,
    view,
    association,
    context: () => context,
    changeContext: async (next: CanvasSourceContext) => {
      context = next;
      for (const listener of [...f.changes])
        listener({ documentId: sourceDocument.id, revision: '2' });
      await settled();
    },
    dispose: () => {
      view.dispose();
      association.dispose();
      binding.dispose();
      f.previews.dispose();
    },
  };
}

test('plugin previews have no click command, border selection semantics or focus stop; native entry fields retain ordered editing', async () => {
  const f = await fixture(true),
    first = f.binding.state.selected;
  f.binding.add();
  await settled();
  const second = f.binding.state.selected,
    row = f.view.element.querySelector<HTMLElement>('[data-input-entry]');
  if (!row) throw new Error('Missing preview.');
  const history = jest.mocked(f.node.graph!.beforeChange!).mock.calls.length;
  const requests = f.execute.mock.calls.length;
  expect(row.tagName).toBe('DIV');
  expect(row.hasAttribute('aria-pressed')).toBe(false);
  expect(row.tabIndex).toBe(-1);
  row.click();
  await settled();
  expect(f.binding.state.selected).toBe(second);
  expect(f.node.graph?.beforeChange).toHaveBeenCalledTimes(history);
  expect(f.execute).toHaveBeenCalledTimes(requests);
  const entry = nativePresentationWidgets(f.node).find((widget) => widget.name.endsWith(':Entry'));
  expect(entry?.type).toBe('combo');
  expect(entry?.options?.hidden).toBe(false);
  entry?.callback?.(first);
  await settled();
  expect(f.binding.state.selected).toBe(first);
  const raster = row.querySelector('canvas');
  publishPluginMode(document, false);
  const ordinary = f.view.element.querySelector<HTMLButtonElement>('[data-input-entry]');
  expect(ordinary?.tagName).toBe('BUTTON');
  expect(ordinary?.querySelector('canvas')).toBe(raster);
  expect(entry?.options?.hidden).toBe(true);
  f.binding.select(second!);
  ordinary?.click();
  expect(f.binding.state.selected).toBe(first);
  f.dispose();
});

test('keeps the exact frame and visible pixels throughout deferred source, file and settings replacements', async () => {
  const f = await fixture(false, true);
  const frame = f.view.element.querySelector<HTMLElement>('.sc-input-editor__frame'),
    canvas = frame?.querySelector('canvas');
  expect(frame?.style.aspectRatio).toBe('3 / 2');
  expect(canvas?.hidden).toBe(false);
  let finish: ((pixels: CanvasPixelRaster) => void) | undefined;
  const hold = new Promise<CanvasPixelRaster>((resolve) => {
    finish = resolve;
  });
  f.files.load.mockImplementationOnce(() => hold);
  f.binding.changeSource({ kind: 'file', file: 'later.png' });
  await settled();
  expect(f.view.element.querySelector('.sc-input-editor__frame')).toBe(frame);
  expect(canvas?.hidden).toBe(false);
  expect(frame?.style.aspectRatio).toBe('3 / 2');
  expect(frame?.getAttribute('aria-busy')).toBe('true');
  finish?.(sourceCoverage);
  await settled();
  expect(canvas?.hidden).toBe(false);
  expect(frame?.getAttribute('aria-busy')).toBe('false');
  f.binding.changeSource({
    kind: 'canvas',
    canvasId: 'editor',
    documentId: null,
    target: { kind: 'current-layer' },
  });
  await settled();
  expect(f.view.element.querySelector('canvas')).toBe(canvas);
  expect(frame?.style.aspectRatio).toBe('3 / 2');
  let interpret: ((pixels: CanvasPixelRaster) => void) | undefined;
  f.files.interpret.mockImplementationOnce(
    () =>
      new Promise((resolve) => {
        interpret = resolve;
      }),
  );
  f.node.widgets![1]!.value = 'blue';
  const changed = f.node.onWidgetChanged;
  if (typeof changed === 'function') changed.call(f.node, 'channel');
  await settled();
  expect(canvas?.hidden).toBe(false);
  expect(frame?.getAttribute('aria-busy')).toBe('true');
  interpret?.(sourceCoverage);
  await settled();
  expect(frame?.getAttribute('aria-busy')).toBe('false');
  f.dispose();
});

test('shows concise missing-layer feedback in the full canvas frame, avoids capture and toast, and recovers after layer selection', async () => {
  const f = await fixture();
  await f.changeContext({ ...f.context(), currentLayerId: null });
  const captures = f.execute.mock.calls.filter(([query]) => query.type === 'capture').length;
  f.binding.changeSource({
    kind: 'canvas',
    canvasId: 'editor',
    documentId: null,
    target: { kind: 'current-layer' },
  });
  await settled();
  const frame = f.view.element.querySelector<HTMLElement>('.sc-input-editor__frame');
  const message = frame?.querySelector('.sc-input-editor__message');
  expect(message?.textContent).toBe('Select a layer');
  expect(frame?.querySelector('[aria-hidden=true]')?.textContent).toBeTruthy();
  expect(frame?.style.aspectRatio).toBe('3 / 2');
  expect(frame?.querySelector('canvas')?.hidden).toBe(true);
  expect(f.execute.mock.calls.filter(([query]) => query.type === 'capture')).toHaveLength(captures);
  expect(f.report).not.toHaveBeenCalled();
  await f.changeContext({ ...f.context(), currentLayerId: 'a' });
  expect(f.view.element.querySelector('.sc-input-editor__frame')).toBe(frame);
  expect(frame?.querySelector('canvas')?.hidden).toBe(false);
  expect(message?.textContent).toBe('');
  expect(f.report).not.toHaveBeenCalled();
  f.dispose();
});

test('a missing file retains its frame and superseded file reads cannot repaint the selected source', async () => {
  const f = await fixture();
  let finish: ((pixels: CanvasPixelRaster) => void) | undefined;
  f.files.load.mockImplementationOnce(
    () =>
      new Promise((resolve) => {
        finish = resolve;
      }),
  );
  f.binding.changeSource({ kind: 'file', file: 'slow.png' });
  await settled();
  f.binding.changeSource({ kind: 'file', file: null });
  await settled();
  finish?.(sourceCoverage);
  await settled();
  const frame = f.view.element.querySelector<HTMLElement>('.sc-input-editor__frame');
  expect(frame?.style.aspectRatio).toBe('3 / 2');
  expect(frame?.querySelector('.sc-input-editor__message')?.textContent).toBe('Choose a file');
  expect(frame?.querySelector('canvas')?.hidden).toBe(true);
  expect(f.report).not.toHaveBeenCalled();
  f.dispose();
});

test('an unavailable ordered target leaves other previews live and still blocks execution', async () => {
  const f = await fixture(true);
  const first = f.binding.state.selected;
  f.binding.add();
  f.binding.changeSource({
    kind: 'canvas',
    canvasId: 'editor',
    documentId: null,
    target: { kind: 'current-layer' },
  });
  await settled();
  await f.changeContext({ ...f.context(), currentLayerId: null });
  const firstRow = [...f.view.element.querySelectorAll<HTMLElement>('[data-input-entry]')].find(
    (row) => row.dataset.inputEntry === first,
  );
  expect(firstRow?.querySelector('canvas')?.hidden).toBe(false);
  expect(f.view.element.textContent).toContain('Select a layer');
  const captures = f.execute.mock.calls.filter(([query]) => query.type === 'capture').length;
  await expect(
    f.capture.capture(
      [
        {
          key: 'queue',
          source: {
            kind: 'canvas',
            canvasId: 'editor',
            documentId: null,
            target: { kind: 'current-layer' },
          },
        },
      ],
      0,
    ),
  ).rejects.toThrow('Select a layer');
  expect(f.execute.mock.calls.filter(([query]) => query.type === 'capture')).toHaveLength(captures);
  expect(f.report).not.toHaveBeenCalled();
  await f.changeContext({ ...f.context(), document: null, layers: [], currentLayerId: null });
  expect(
    f.view.element.querySelector('.sc-input-editor__frame')?.getAttribute('data-content'),
  ).toBe('black');
  expect(
    f.view.element.querySelector<HTMLElement>('.sc-input-editor__frame')?.style.aspectRatio,
  ).toBe('3 / 2');
  expect(f.report).not.toHaveBeenCalled();
  f.dispose();
});

test('reserves a full canvas placeholder after the final entry is removed and keeps commands below the preview', async () => {
  const f = await fixture(true);
  f.binding.remove();
  await settled();
  const empty = f.view.element.querySelector<HTMLElement>('[data-input-empty]');
  expect(empty?.hidden).toBe(false);
  expect(empty?.querySelector<HTMLElement>('.sc-input-editor__frame')?.style.aspectRatio).toBe(
    '3 / 2',
  );
  expect(empty?.querySelector('.sc-input-editor__frame')?.getAttribute('data-content')).toBe(
    'black',
  );
  expect(f.view.element.querySelector('.sc-input-editor__rows')?.nextElementSibling).toBe(
    f.view.controlsElement,
  );
  f.binding.add();
  await settled();
  expect(empty?.hidden).toBe(true);
  expect(f.view.element.querySelectorAll('[data-input-entry]')).toHaveLength(1);
  expect(f.report).not.toHaveBeenCalled();
  f.dispose();
});

test('restores an unchosen file source as a canvas-shaped placeholder after remount', async () => {
  const f = await fixture();
  f.binding.changeSource({ kind: 'file', file: null });
  f.view.dispose();
  const restored = new MasonryInputEditor(
    document,
    f.binding,
    f.previews,
    async () => 'chosen.png',
    f.report,
    nativeInputFields(f.node),
    document.createElement('article'),
    f.association.source,
  );
  document.body.append(restored.element);
  await settled();
  expect(
    restored.element.querySelector<HTMLElement>('[data-input-entry] .sc-input-editor__frame')?.style
      .aspectRatio,
  ).toBe('3 / 2');
  expect(
    restored.element.querySelector('[data-input-entry] .sc-input-editor__message')?.textContent,
  ).toBe('Choose a file');
  expect(f.report).not.toHaveBeenCalled();
  restored.dispose();
  f.binding.dispose();
  f.previews.dispose();
});

test('keeps loader cards present while native fields are replaced by local source controls', () => {
  const node: ComfyNode = {
    id: 4,
    type: 'LoadImage',
    widgets: [{ name: 'image', type: 'combo', value: '' }],
    inputs: [],
    outputs: [],
  };
  const state = createDefaultCubeSurfaceState(),
    visible = () => resolveCubeFaceCardPresentation([node], state).cards[0]?.visible;
  expect(visible()).toBe(true);
  const root = document.createElement('article');
  document.body.append(root);
  const replacement = new InputLoaderPresentation(node, root, ['image']);
  expect(visible()).toBe(true);
  replacement.dispose();
  expect(visible()).toBe(true);
});
