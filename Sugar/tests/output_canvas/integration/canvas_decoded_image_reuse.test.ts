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
/** Prove decoded output images survive attachment and workflow view replacement. */
import { expect, test } from '@jest/globals';
import { MemoryBudgetRegistry } from '../../../frontend/comfyui/ui/core/memory/MemoryBudgetRegistry.js';
import { BrowserCanvasDecodedImages } from '../../../frontend/comfyui/ui/output_canvas/infrastructure/BrowserCanvasDecodedImages.js';
import type { BrowserCanvasImagePayload } from '../../../frontend/comfyui/ui/output_canvas/infrastructure/BrowserCanvasImageLoader.js';
import type {
  CanvasImageInput,
  CanvasImageLoad,
} from '../../../frontend/comfyui/ui/output_canvas/domain/CanvasImageResource.js';

/** Decode synchronously while counting platform decodes and destroyed images. */
function platform() {
  const destroyed: string[] = [];
  let decodes = 0;
  return {
    destroyed,
    decodes: () => decodes,
    disposed: false,
    start(input: CanvasImageInput<BrowserCanvasImagePayload>): CanvasImageLoad<HTMLImageElement> {
      decodes += 1;
      const handle = document.createElement('img');
      return {
        completion: Promise.resolve({
          handle,
          dimensions: { width: 1024, height: 1024 },
          release: () => destroyed.push(input.key),
        }),
        cancel: () => undefined,
      };
    },
    dispose(): void {
      this.disposed = true;
    },
  };
}

/** Build one main-image admission for an output item. */
function input(key: string, preview = false): CanvasImageInput<BrowserCanvasImagePayload> {
  return {
    key,
    runOrder: 1,
    selection: { sourceId: 'source:test', batchIndex: 0 },
    lane: key,
    fallbackKey: null,
    dimensions: { width: 1024, height: 1024 },
    role: 'main',
    preview,
    payload: preview
      ? { kind: 'preview', blob: new Blob(['frame']) }
      : { kind: 'file', url: `/view?filename=${key}.png` },
  };
}

test('reuses decoded images across repeated attachment replacement without decoding again', async () => {
  const loader = platform();
  const images = new BrowserCanvasDecodedImages(loader, new MemoryBudgetRegistry());
  let first: HTMLImageElement | null = null;
  for (let cycle = 0; cycle < 10; cycle++) {
    const attachment = images.attachment();
    const decoded = await attachment.start(input('item')).completion;
    first ??= decoded.handle;
    expect(decoded.handle).toBe(first);
    // A view change disposes the attachment; delivered images return to the shared cache.
    attachment.dispose();
  }
  expect(loader.decodes()).toBe(1);
  expect(loader.destroyed).toEqual([]);
  expect(images.diagnostics()).toEqual({ parkedImages: 1, parkedBytes: 1024 * 1024 * 4 });
  images.dispose();
  expect(loader.destroyed).toEqual(['item']);
  expect(loader.disposed).toBe(true);
});

test('never parks live preview frames and destroys them on release', async () => {
  const loader = platform();
  const images = new BrowserCanvasDecodedImages(loader, new MemoryBudgetRegistry());
  const attachment = images.attachment();
  const frame = await attachment.start(input('frame', true)).completion;
  frame.release();
  expect(loader.destroyed).toEqual(['frame']);
  expect(images.diagnostics().parkedImages).toBe(0);
});

test('releases parked images under memory pressure while displayed images stay with their view', async () => {
  const loader = platform();
  const memory = new MemoryBudgetRegistry();
  const images = new BrowserCanvasDecodedImages(loader, memory);
  const attachment = images.attachment();
  const hidden = await attachment.start(input('hidden')).completion;
  const shown = await attachment.start(input('shown')).completion;
  hidden.release();
  memory.setAllowance(64 * 1024 * 1024, 'critical');
  expect(loader.destroyed).toEqual(['hidden']);
  expect(shown.handle).toBeInstanceOf(HTMLImageElement);
  expect(memory.usage().consumers).toContainEqual({
    id: 'output-canvas-decoded-images',
    bytes: 0,
  });
});
