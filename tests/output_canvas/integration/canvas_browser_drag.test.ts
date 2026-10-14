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
/** Prove exact output dragging retains one preparation and cannot export obsolete pixels. */
import { jest } from '@jest/globals';
import { BrowserCanvasDrag } from '../../../frontend/comfyui/ui/output_canvas/presentation/browser/BrowserCanvasDrag.js';

/** Model only the synchronous browser DataTransfer boundary. */
function drag(image: HTMLImageElement) {
  const values = new Map<string, string>();
  const files: File[] = [new File(['native preview'], 'view', { type: 'image/png' })];
  const transfer = {
    setData: (type: string, value: string) => values.set(type, value),
    items: {
      add: (file: File) => files.push(file),
      clear: () => {
        files.splice(0);
        values.clear();
      },
    },
    effectAllowed: 'none',
  };
  const event = new MouseEvent('dragstart', { bubbles: true, cancelable: true });
  Object.defineProperty(event, 'dataTransfer', { value: transfer });
  image.dispatchEvent(event);
  return { event, values, files, transfer };
}

/** Observe the bounded asynchronous preparation without depending on browser network scheduling. */
async function settled(): Promise<void> {
  await Promise.resolve();
  await Promise.resolve();
}

test('exports the exact prepared file and download URL and releases it on leaving', async () => {
  const root = document.createElement('div');
  const image = document.createElement('img');
  image.draggable = false;
  root.append(image);
  const file = new File(['jpeg bytes'], '<output>.jpg', { type: 'image/jpeg' });
  const prepare = jest
    .fn<(key: string, signal: AbortSignal) => Promise<{ file: File; url: string }>>()
    .mockResolvedValue({ file, url: 'http://localhost/view?filename=output.jpg' });
  const owner = new BrowserCanvasDrag(root, {
    resolve: (target) => (target === image ? { key: 'batch-2', element: image } : null),
    prepare,
    accepts: () => true,
    report: jest.fn(),
  });
  image.dispatchEvent(new MouseEvent('pointerover', { bubbles: true }));
  await settled();
  expect(image.draggable).toBe(true);
  image.dispatchEvent(new MouseEvent('pointermove', { bubbles: true }));
  expect(prepare).toHaveBeenCalledTimes(1);
  const result = drag(image);
  expect(result.event.defaultPrevented).toBe(false);
  expect(result.files).toEqual([file]);
  expect(result.values.get('DownloadURL')).toBe(
    'image/jpeg:<output>.jpg:http://localhost/view?filename=output.jpg',
  );
  expect(result.values.get('text/uri-list')).toBe('http://localhost/view?filename=output.jpg');
  expect(result.transfer.effectAllowed).toBe('copy');
  root.dispatchEvent(new Event('pointerleave'));
  expect(image.draggable).toBe(false);
  expect(drag(image).event.defaultPrevented).toBe(true);
  owner.dispose();
});

test('cancels pending work and rejects a late preparation after the displayed batch changes', async () => {
  const root = document.createElement('div');
  const image = document.createElement('img');
  root.append(image);
  let current = 'first';
  let finish: ((value: { file: File; url: string }) => void) | undefined;
  let signal: AbortSignal | undefined;
  const owner = new BrowserCanvasDrag(root, {
    resolve: (target) => (target === image ? { key: current, element: image } : null),
    prepare: (_key, request) => {
      signal = request;
      return new Promise((resolve) => {
        finish = resolve;
      });
    },
    accepts: () => true,
    report: jest.fn(),
  });
  image.dispatchEvent(new MouseEvent('pointerover', { bubbles: true }));
  expect(image.draggable).toBe(false);
  expect(owner.allows(new MouseEvent('pointerdown'))).toBe(false);
  expect(drag(image).event.defaultPrevented).toBe(true);
  current = 'second';
  owner.reconcile();
  expect(signal?.aborted).toBe(true);
  finish?.({
    file: new File(['old'], 'old.png', { type: 'image/png' }),
    url: 'http://localhost/old',
  });
  await settled();
  expect(drag(image).event.defaultPrevented).toBe(true);
  owner.dispose();
});

test('admits outbound navigation only after full image bytes are ready for that exact element', async () => {
  const root = document.createElement('div');
  const image = document.createElement('img');
  root.append(image);
  let finish: ((value: { file: File; url: string }) => void) | undefined;
  const owner = new BrowserCanvasDrag(root, {
    resolve: (target) => (target === image ? { key: 'batch-3', element: image } : null),
    prepare: () =>
      new Promise((resolve) => {
        finish = resolve;
      }),
    accepts: () => true,
    report: jest.fn(),
  });
  const press = new MouseEvent('pointerdown');
  Object.defineProperty(press, 'target', { value: image });
  image.dispatchEvent(new MouseEvent('pointerover', { bubbles: true }));
  expect(image.draggable).toBe(false);
  expect(owner.allows(press)).toBe(false);
  finish?.({
    file: new File(['full image'], 'batch-3.png', { type: 'image/png' }),
    url: 'http://localhost/full.png',
  });
  await settled();
  expect(image.draggable).toBe(true);
  expect(owner.allows(press)).toBe(true);
  owner.invalidate();
  expect(owner.allows(press)).toBe(false);
  expect(image.draggable).toBe(false);
  owner.dispose();
});

test('reports current preparation failures once and silently retires cancellations', async () => {
  const root = document.createElement('div');
  const image = document.createElement('img');
  root.append(image);
  const report = jest.fn<(error: unknown) => void>();
  const owner = new BrowserCanvasDrag(root, {
    resolve: (target) => (target === image ? { key: 'output', element: image } : null),
    prepare: async () => {
      throw new Error('File unavailable');
    },
    accepts: () => true,
    report,
  });
  image.dispatchEvent(new MouseEvent('pointerover', { bubbles: true }));
  await settled();
  expect(report).toHaveBeenCalledTimes(1);
  expect(drag(image).event.defaultPrevented).toBe(true);
  owner.dispose();
  image.dispatchEvent(new MouseEvent('pointerover', { bubbles: true }));
  expect(report).toHaveBeenCalledTimes(1);
});
