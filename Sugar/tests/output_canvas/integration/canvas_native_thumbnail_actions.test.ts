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
/** Prove native image actions preserve controls and stay scoped to attributable completed outputs. */
import { jest } from '@jest/globals';
import { canvasCatalogFixture } from '../support/CanvasCatalogFixture.js';
import { canvasRevealTarget } from '../../../frontend/comfyui/ui/output_canvas/domain/CanvasReveal.js';
import { ComfyImageRevealHost } from '../../../frontend/comfyui/ui/output_canvas/infrastructure/ComfyImageRevealHost.js';
import { ComfyImageRevealMenus } from '../../../frontend/comfyui/ui/output_canvas/infrastructure/ComfyImageRevealMenus.js';
import { resolveComfyImageReveal } from '../../../frontend/comfyui/ui/output_canvas/infrastructure/ComfyImageRevealTarget.js';
import type { UnknownRecord } from '../../../frontend/comfyui/ui/types/common.js';
import { ComfyImageRevealDom } from '../../../frontend/comfyui/ui/output_canvas/infrastructure/ComfyImageRevealDom.js';

/** Flush browser mount observation without running graph discovery or a recurring clock. */
async function settle(): Promise<void> {
  await Promise.resolve();
  await Promise.resolve();
}

afterEach(() => {
  document.body.replaceChildren();
  jest.restoreAllMocks();
});

test('adds one action per real batch image and preserves native gallery/download identity', async () => {
  const catalog = canvasCatalogFixture();
  const source = catalog.entries[0]!.source;
  const node: UnknownRecord = { id: '1' };
  const card = document.createElement('section');
  card.dataset.nodeId = '1';
  const root = document.createElement('div');
  root.className = 'image-preview';
  const native = document.createElement('button');
  native.setAttribute('aria-label', 'Download image');
  const download = jest.fn();
  native.addEventListener('click', download);
  const image = document.createElement('img');
  image.src = '/api/view?filename=1.png&type=output&preview=webp%3B75';
  root.append(image, native);
  card.append(root);
  document.body.append(card);
  const reveal = jest.fn(async () => true);
  const host = new ComfyImageRevealHost({
    document,
    app: { ui: { settings: { getSettingValue: () => true } } },
    logger: console,
    catalog: () => catalog,
    sources: () => [{ path: source.executionPath, node }],
    subscribe: () => () => {},
    reveal,
  });
  host.setup();
  try {
    const action = root.querySelector<HTMLButtonElement>('[data-sc-image-reveal]')!;
    expect(action).not.toBeNull();
    action.click();
    expect(reveal).toHaveBeenCalledWith(canvasRevealTarget(catalog.entries[0]!.items[0]!));
    native.click();
    expect(download).toHaveBeenCalledTimes(1);
    const popup = document.createElement('aside');
    popup.textContent = '<img src=x onerror=bad()>';
    document.body.append(popup);
    await settle();
    expect(root.querySelector('[data-sc-image-reveal]')).toBe(action);
    expect(root.querySelector('button[aria-label="Download image"]')).toBe(native);
    expect(root.querySelector('img')).toBe(image);
    expect(popup.querySelector('img')).toBeNull();
    image.src = 'blob:transient-latent-preview';
    await settle();
    expect(root.querySelector('[data-sc-image-reveal]')).toBeNull();
  } finally {
    host.dispose();
  }
  expect(node.getExtraMenuOptions).toBeUndefined();
});

test('ignores unrelated popup mounts and refreshes only the changed native surface', async () => {
  const catalog = canvasCatalogFixture();
  const roots = ['1', '7:23'].map((id) => {
    const card = document.createElement('section');
    card.dataset.nodeId = id;
    const root = document.createElement('div');
    root.className = 'image-preview';
    const image = document.createElement('img');
    image.src = '/api/view?filename=1.png&type=output';
    root.append(image);
    card.append(root);
    document.body.append(card);
    return root;
  });
  const sources = catalog.entries.map((entry) => ({
    path: entry.source.executionPath,
    node: { id: entry.source.executionPath },
  }));
  const inventory = jest.fn(() => sources);
  const host = new ComfyImageRevealHost({
    document,
    app: { ui: { settings: { getSettingValue: () => true } } },
    logger: console,
    catalog: () => catalog,
    sources: inventory,
    subscribe: () => () => {},
    reveal: async () => true,
  });
  host.setup();
  await settle();
  inventory.mockClear();
  const refresh = jest.spyOn(ComfyImageRevealDom.prototype, 'refresh');
  try {
    const unaffected = roots[1]!.querySelector('[data-sc-image-reveal]');
    const popup = document.createElement('div');
    popup.setAttribute('role', 'listbox');
    document.body.append(popup);
    await settle();
    expect(inventory).not.toHaveBeenCalled();
    expect(refresh).not.toHaveBeenCalled();
    roots[0]!.querySelector('img')!.src = 'blob:new-preview';
    await settle();
    expect(roots[0]!.querySelector('[data-sc-image-reveal]')).toBeNull();
    expect(roots[1]!.querySelector('[data-sc-image-reveal]')).toBe(unaffected);
    expect(refresh).toHaveBeenCalledTimes(1);
  } finally {
    host.dispose();
  }
});

test('retains native menu entries and restores borrowed callbacks with their original receiver', () => {
  const catalog = canvasCatalogFixture();
  const target = canvasRevealTarget(catalog.entries[0]!.items[0]!);
  const reveal = jest.fn();
  const native = jest.fn(function (this: UnknownRecord, _canvas: unknown, options: unknown[]) {
    expect(this.id).toBe('1');
    options.push({ content: 'Download image' });
    return 'native';
  });
  const node: UnknownRecord = { id: '1', getExtraMenuOptions: native };
  const menus = new ComfyImageRevealMenus(() => target, reveal);
  menus.reconcile([{ path: '1', node }]);
  menus.context('1', target);
  const options: { content: string; callback?: () => void }[] = [];
  if (typeof node.getExtraMenuOptions !== 'function')
    throw new Error('Native menu callback is missing');
  expect(Reflect.apply(node.getExtraMenuOptions, node, [{}, options])).toBe('native');
  expect(options.map((item) => item.content)).toEqual(['Download image', 'Show in output canvas']);
  options[1]!.callback!();
  expect(reveal).toHaveBeenCalledWith(target);
  menus.dispose();
  expect(node.getExtraMenuOptions).toBe(native);
});

test('normalizes thumbnail URL encodings without accepting another source or a transient preview', () => {
  const catalog = canvasCatalogFixture();
  const origin = document.location.origin;
  expect(
    resolveComfyImageReveal(
      catalog,
      '1',
      `${origin}/api/view?filename=1.png&type=output&rand=2&preview=webp`,
      origin,
      null,
    ),
  ).not.toBeNull();
  for (const url of [
    'blob:preview',
    'https://other.invalid/view?filename=1.png',
    '/api/view?filename=1.png&type=temp',
    '/api/view?filename=other.png',
  ])
    expect(resolveComfyImageReveal(catalog, '1', url, origin, null)).toBeNull();
  expect(
    resolveComfyImageReveal(
      catalog,
      'missing',
      '/api/view?filename=1.png&type=output',
      origin,
      null,
    ),
  ).toBeNull();
});

test('refuses an ambiguous repeated file rather than revealing the wrong batch position', () => {
  const catalog = canvasCatalogFixture();
  const entry = catalog.entries[0]!;
  const item = entry.items[0]!;
  const duplicated = {
    ...catalog,
    result: {
      ...catalog.result,
      outputs: catalog.result.outputs.map((output) =>
        output.source.id === entry.source.id
          ? {
              ...output,
              items: [item, { ...item, id: 'result:duplicate' as const, batchIndex: 1 }],
            }
          : output,
      ),
    },
    entries: [
      { ...entry, items: [item, { ...item, id: 'result:duplicate' as const, batchIndex: 1 }] },
    ],
  };
  expect(
    resolveComfyImageReveal(
      duplicated,
      '1',
      '/api/view?filename=1.png&type=output',
      document.location.origin,
      null,
    ),
  ).toBeNull();
  expect(
    resolveComfyImageReveal(
      duplicated,
      '1',
      '/api/view?filename=1.png&type=output',
      document.location.origin,
      1,
    ),
  ).toMatchObject({ itemId: 'result:duplicate', batchIndex: 1 });
});
