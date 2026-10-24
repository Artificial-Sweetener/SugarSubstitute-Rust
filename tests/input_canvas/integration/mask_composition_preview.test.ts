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
/** Prove effective masks overlay coherent composition context through the existing source and field owners. */
import { jest } from '@jest/globals';
import { randomUUID } from 'node:crypto';
import {
  sourceMaskFixture,
  sourceDocument,
  sourceCoverage,
} from '../support/source_mask_fixture.js';
import { nativeInputFields } from '../support/native_input_fields.js';
import { encodeCanvasRaster } from '../../../integrations/shared/CanvasRasterEncoding.js';
import { createComfyInputBinding } from '../../../frontend/comfyui/ui/input_canvas/infrastructure/ComfyInputBinding.js';
import { MasonryInputEditor } from '../../../frontend/comfyui/ui/input_canvas/presentation/MasonryInputEditor.js';
import { publishPluginMode } from '../../../frontend/comfyui/ui/core/PluginMode.js';
import type { ComfyNode } from '../../../frontend/comfyui/ui/types/graph.js';

beforeAll(() =>
  Object.defineProperty(crypto, 'randomUUID', { configurable: true, value: randomUUID }),
);
afterEach(() => {
  publishPluginMode(document, false);
  document.body.replaceChildren();
  jest.restoreAllMocks();
});

/** Settle shared source batching and interpretation without native UI or timer-driven capture. */
async function settled(): Promise<void> {
  for (let index = 0; index < 150; index++) await Promise.resolve();
}

/** Keep canvas capture and field interpretation real, replacing only native pixels and the DOM raster boundary. */
async function fixture(custom = false) {
  const f = sourceMaskFixture(),
    original = f.execute.getMockImplementation();
  if (!original) throw new Error('Missing native boundary.');
  const composition = encodeCanvasRaster(
    3,
    2,
    'rgba8',
    Uint8Array.from(Array.from({ length: 6 }, () => [80, 120, 160, 255]).flat()),
  );
  const layer = encodeCanvasRaster(
    3,
    2,
    'rgba8',
    Uint8Array.from(Array.from({ length: 6 }, () => [0, 255, 128, 64]).flat()),
  );
  let splitRevision = false;
  f.execute.mockImplementation(async (query) => {
    const value = await original(query);
    if (value.kind !== 'captured' || query.type !== 'capture') return value;
    return {
      ...value,
      snapshots: value.snapshots.map((snapshot) => ({
        ...snapshot,
        revision: splitRevision && query.maximumSize === 0 ? 'old' : 'current',
        pixels:
          snapshot.target.kind === 'composition'
            ? composition
            : snapshot.target.kind === 'selection'
              ? sourceCoverage
              : layer,
      })),
    };
  });
  const painted: Uint8ClampedArray[] = [];
  jest.spyOn(HTMLCanvasElement.prototype, 'getContext').mockReturnValue({
    createImageData: (width: number, height: number) => ({
      data: new Uint8ClampedArray(width * height * 4),
    }),
    putImageData: (image: { data: Uint8ClampedArray }) => painted.push(image.data.slice()),
  } as unknown as CanvasRenderingContext2D);
  publishPluginMode(document, true);
  const node: ComfyNode = {
    id: 4,
    type: 'LoadImageMask',
    properties: {},
    widgets: [
      { name: 'image', value: '' },
      { name: 'channel', value: 'red' },
    ],
  };
  const binding = createComfyInputBinding(
    node,
    {
      kind: 'mask',
      classType: custom ? 'CustomMask' : 'LoadImageMask',
      field: 'image',
      channel: 'channel',
      ordered: false,
    },
    f.ports,
  );
  await binding.initialize();
  let view: MasonryInputEditor | null = null;
  return {
    ...f,
    node,
    binding,
    painted,
    mount: async () => {
      view = new MasonryInputEditor(
        document,
        binding,
        f.previews,
        async () => 'mask.png',
        f.report,
        nativeInputFields(node),
        document.createElement('article'),
      );
      document.body.append(view.element);
      await settled();
      return view;
    },
    channel: async (channel: string) => {
      node.widgets![1]!.value = channel;
      if (typeof node.onWidgetChanged === 'function') node.onWidgetChanged.call(node, 'channel');
      await settled();
    },
    split: (value: boolean) => {
      splitRevision = value;
    },
    changed: async () => {
      for (const listener of [...f.changes])
        listener({ documentId: sourceDocument.id, revision: 'current' });
      await settled();
    },
    dispose: () => {
      view?.dispose();
      binding.dispose();
      f.previews.dispose();
    },
  };
}

test('shows one black/white wash over the full composition and reinterprets channels without recapturing it', async () => {
  const f = await fixture(),
    view = await f.mount();
  const frame = view.element.querySelector('.sc-input-editor__frame'),
    canvas = view.element.querySelector('canvas');
  expect([...f.painted.at(-1)!.slice(0, 8)]).toEqual([40, 60, 80, 255, 168, 188, 208, 255]);
  const captures = f.execute.mock.calls.filter(([query]) => query.type === 'capture');
  expect(captures).toHaveLength(1);
  expect(captures[0]?.[0]).toMatchObject({
    targets: [{ kind: 'composition' }, { kind: 'selection' }],
    maximumSize: 256,
  });
  f.binding.changeSource({
    kind: 'canvas',
    canvasId: 'editor',
    documentId: null,
    target: { kind: 'current-layer' },
  });
  await settled();
  const count = f.execute.mock.calls.filter(([query]) => query.type === 'capture').length;
  for (const [channel, expected] of [
    ['red', [40, 60, 80, 255]],
    ['green', [168, 188, 208, 255]],
    ['blue', [104, 124, 144, 255]],
    ['alpha', [136, 156, 176, 255]],
  ] as const) {
    await f.channel(channel);
    expect([...f.painted.at(-1)!.slice(0, 4)]).toEqual(expected);
  }
  expect(f.execute.mock.calls.filter(([query]) => query.type === 'capture')).toHaveLength(count);
  expect(view.element.querySelector('.sc-input-editor__frame')).toBe(frame);
  expect(view.element.querySelector('canvas')).toBe(canvas);
  expect(f.stage).not.toHaveBeenCalled();
  expect(f.report).not.toHaveBeenCalled();
  f.dispose();
});

test('reuses interpreted file masks while composition updates repaint the persistent preview', async () => {
  const f = await fixture(),
    view = await f.mount();
  const canvas = view.element.querySelector('canvas');
  f.binding.changeSource({ kind: 'file', file: 'mask.png' });
  await settled();
  expect([...f.painted.at(-1)!.slice(0, 4)]).toEqual([40, 60, 80, 255]);
  const decodes = f.files.load.mock.calls.length,
    paints = f.painted.length;
  await f.changed();
  expect(f.painted.length).toBeGreaterThan(paints);
  expect(f.files.load).toHaveBeenCalledTimes(decodes);
  expect(view.element.querySelector('canvas')).toBe(canvas);
  expect(f.report).not.toHaveBeenCalled();
  f.dispose();
});

test('keeps custom-loader interpretation separate from a bounded composition and waits for matching revisions', async () => {
  const f = await fixture(true);
  f.split(true);
  const view = await f.mount();
  expect(f.painted).toHaveLength(0);
  expect(view.element.querySelector('canvas')?.hidden).toBe(true);
  expect(
    f.execute.mock.calls.flatMap(([query]) =>
      query.type === 'capture' ? [query.maximumSize] : [],
    ),
  ).toEqual([256, 0]);
  f.split(false);
  await f.changed();
  expect(f.painted.length).toBeGreaterThan(0);
  expect([...f.painted.at(-1)!.slice(0, 4)]).toEqual([40, 60, 80, 255]);
  expect(f.report).not.toHaveBeenCalled();
  f.dispose();
});
