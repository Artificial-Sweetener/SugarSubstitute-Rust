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
/** Prove retained rendition cancellation and bounded browser codec allocation lifetime. */
import { jest } from '@jest/globals';
import { BrowserImageRendition } from '../../../frontend/comfyui/ui/output_saving/BrowserImageRendition.js';

test.each([false, true])(
  'releases bitmap/canvas after conversion with caller cancellation=%s',
  async (cancelled) => {
    const descriptor = Object.getOwnPropertyDescriptor(globalThis, 'createImageBitmap');
    const close = jest.fn();
    const draw = jest.fn();
    const controller = new AbortController();
    // Browser drawing is an external adapter; supply only the consumed drawing operation.
    const context = jest
      .spyOn(HTMLCanvasElement.prototype, 'getContext')
      .mockReturnValue({ drawImage: draw } as unknown as CanvasRenderingContext2D);
    const canvases: HTMLCanvasElement[] = [];
    const encoder = jest.spyOn(HTMLCanvasElement.prototype, 'toBlob').mockImplementation(function (
      this: HTMLCanvasElement,
      callback,
      mime,
      quality,
    ) {
      canvases.push(this);
      expect(mime).toBe('image/jpeg');
      expect(quality).toBe(0.73);
      if (cancelled) controller.abort();
      callback(new Blob(['encoded'], { type: 'image/jpeg' }));
    });
    Object.defineProperty(globalThis, 'createImageBitmap', {
      configurable: true,
      value: async () => ({ width: 4, height: 3, close }),
    });
    try {
      const pending = new BrowserImageRendition(document).encode(
        new Blob(['original'], { type: 'image/png' }),
        'image/jpeg',
        0.73,
        { width: 4, height: 3 },
        controller.signal,
      );
      if (cancelled) await expect(pending).rejects.toThrow('Cancelled');
      else expect((await pending).type).toBe('image/jpeg');
      expect(draw).toHaveBeenCalledTimes(1);
      expect(close).toHaveBeenCalledTimes(1);
      expect(canvases[0]?.width).toBe(0);
      expect(canvases[0]?.height).toBe(0);
    } finally {
      context.mockRestore();
      encoder.mockRestore();
      if (descriptor) Object.defineProperty(globalThis, 'createImageBitmap', descriptor);
      else Reflect.deleteProperty(globalThis, 'createImageBitmap');
    }
  },
);

test('rejects decoded geometry mismatch and releases the real bitmap ownership', async () => {
  const descriptor = Object.getOwnPropertyDescriptor(globalThis, 'createImageBitmap');
  const close = jest.fn();
  Object.defineProperty(globalThis, 'createImageBitmap', {
    configurable: true,
    value: async () => ({ width: 9000, height: 8000, close }),
  });
  try {
    await expect(
      new BrowserImageRendition(document).encode(
        new Blob(['source'], { type: 'image/png' }),
        'image/png',
        1,
        { width: 4, height: 3 },
      ),
    ).rejects.toThrow('geometry');
    expect(close).toHaveBeenCalledTimes(1);
  } finally {
    if (descriptor) Object.defineProperty(globalThis, 'createImageBitmap', descriptor);
    else Reflect.deleteProperty(globalThis, 'createImageBitmap');
  }
});

test('refuses an unbounded encoded or decoded source before invoking a browser bitmap allocation', async () => {
  const descriptor = Object.getOwnPropertyDescriptor(globalThis, 'createImageBitmap');
  const decode = jest.fn();
  Object.defineProperty(globalThis, 'createImageBitmap', { configurable: true, value: decode });
  try {
    await expect(
      new BrowserImageRendition(document).encode(
        new Blob(['source'], { type: 'image/png' }),
        'image/png',
        1,
        { width: 30000, height: 30000 },
      ),
    ).rejects.toThrow('bounds');
    expect(decode).not.toHaveBeenCalled();
  } finally {
    if (descriptor) Object.defineProperty(globalThis, 'createImageBitmap', descriptor);
    else Reflect.deleteProperty(globalThis, 'createImageBitmap');
  }
});
