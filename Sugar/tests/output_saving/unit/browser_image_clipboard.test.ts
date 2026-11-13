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
/** Prove browser clipboard adaptation preserves exact preferred bytes and releases decoded pixels. */
import { jest } from '@jest/globals';
import { BrowserImageClipboard } from '../../../frontend/comfyui/ui/output_saving/BrowserImageClipboard.js';

test('copies canonical PNG bytes directly without an additional pixel decode', async () => {
  const descriptor = Object.getOwnPropertyDescriptor(globalThis, 'ClipboardItem');
  const clipboardDescriptor = Object.getOwnPropertyDescriptor(navigator, 'clipboard');
  let offered: Record<string, Blob | Promise<Blob>> | null = null;
  class Item {
    /** Capture the external browser constructor payload for identity assertions. */
    constructor(value: Record<string, Blob | Promise<Blob>>) {
      offered = value;
    }
    /** Model Chromium\'s interoperable image support without admitting JPEG as a native format. */
    static supports(type: string): boolean {
      return type === 'image/png';
    }
  }
  const write = jest.fn<(items: readonly object[]) => Promise<void>>().mockResolvedValue(undefined);
  Object.defineProperty(globalThis, 'ClipboardItem', { configurable: true, value: Item });
  Object.defineProperty(navigator, 'clipboard', { configurable: true, value: { write } });
  try {
    const blob = new Blob(['canonical'], { type: 'image/png' });
    await new BrowserImageClipboard(document).write(blob, 'image/png');
    expect(offered).toEqual({ 'image/png': blob });
    expect(write).toHaveBeenCalledTimes(1);
  } finally {
    if (descriptor) Object.defineProperty(globalThis, 'ClipboardItem', descriptor);
    else Reflect.deleteProperty(globalThis, 'ClipboardItem');
    if (clipboardDescriptor) Object.defineProperty(navigator, 'clipboard', clipboardDescriptor);
    else Reflect.deleteProperty(navigator, 'clipboard');
  }
});

test('reports unavailable clipboard support without native dialogs or pretending copying succeeded', async () => {
  await expect(
    new BrowserImageClipboard(document).write(new Blob(['pixels']), 'image/png'),
  ).rejects.toThrow('unavailable');
});

test.each([true, false])(
  'offers JPEG pixels and releases decoding with custom JPEG support=%s',
  async (supportsCustom) => {
    const itemDescriptor = Object.getOwnPropertyDescriptor(globalThis, 'ClipboardItem');
    const bitmapDescriptor = Object.getOwnPropertyDescriptor(globalThis, 'createImageBitmap');
    const clipboardDescriptor = Object.getOwnPropertyDescriptor(navigator, 'clipboard');
    const payload: { formats: Record<string, Blob | Promise<Blob>> } = { formats: {} };
    class Item {
      /** Capture portable bytes rather than implementing browser clipboard behavior in the fixture. */
      constructor(formats: Record<string, Blob | Promise<Blob>>) {
        payload.formats = formats;
      }
      /** Admit the actual Chromium formats observed in the in-app browser. */
      static supports(type: string): boolean {
        return type === 'image/png' || (supportsCustom && type === 'web image/jpeg');
      }
    }
    const close = jest.fn();
    const draw = jest.fn();
    // Canvas drawing is an external browser capability; this fixture implements only the consumed method.
    const context = jest
      .spyOn(HTMLCanvasElement.prototype, 'getContext')
      .mockReturnValue({ drawImage: draw } as unknown as CanvasRenderingContext2D);
    const encoded = new Blob(['PNG pixels'], { type: 'image/png' });
    const canvases: HTMLCanvasElement[] = [];
    const encode = jest.spyOn(HTMLCanvasElement.prototype, 'toBlob').mockImplementation(function (
      this: HTMLCanvasElement,
      callback,
    ) {
      canvases.push(this);
      callback(encoded);
    });
    Object.defineProperty(globalThis, 'ClipboardItem', { configurable: true, value: Item });
    Object.defineProperty(globalThis, 'createImageBitmap', {
      configurable: true,
      value: async () => ({ width: 4, height: 3, close }),
    });
    Object.defineProperty(navigator, 'clipboard', {
      configurable: true,
      value: {
        write: async () => {
          await Promise.all(Object.values(payload.formats).map((value) => Promise.resolve(value)));
        },
      },
    });
    try {
      const jpeg = new Blob(['exact JPEG'], { type: 'image/jpeg' });
      await new BrowserImageClipboard(document).write(jpeg, 'image/jpeg');
      expect(payload.formats['web image/jpeg']).toBe(supportsCustom ? jpeg : undefined);
      expect(await payload.formats['image/png']).toBe(encoded);
      expect(draw).toHaveBeenCalledTimes(1);
      expect(close).toHaveBeenCalledTimes(1);
      expect(canvases[0]?.width).toBe(0);
    } finally {
      context.mockRestore();
      encode.mockRestore();
      for (const [name, descriptor] of [
        ['ClipboardItem', itemDescriptor],
        ['createImageBitmap', bitmapDescriptor],
      ] as const) {
        if (descriptor) Object.defineProperty(globalThis, name, descriptor);
        else Reflect.deleteProperty(globalThis, name);
      }
      if (clipboardDescriptor) Object.defineProperty(navigator, 'clipboard', clipboardDescriptor);
      else Reflect.deleteProperty(navigator, 'clipboard');
    }
  },
);
