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
/** Prove generation transfer normalizes intrinsic pixels and releases browser raster resources. */
import { jest } from '@jest/globals';
import { BrowserCanvasGenerationImages } from '../../../frontend/comfyui/ui/execution/infrastructure/BrowserCanvasGenerationImages.js';
import { CanvasCancellation } from '../../../integrations/shared/CanvasCancellation.js';
import { decodeCanvasImage } from '../../../integrations/shared/CanvasImageEncoding.js';

const browserProperties = new Map(
  ['createImageBitmap', 'OffscreenCanvas'].map((name) => [
    name,
    Object.getOwnPropertyDescriptor(globalThis, name),
  ]),
);

afterEach(() => {
  for (const [name, descriptor] of browserProperties) {
    if (descriptor) Object.defineProperty(globalThis, name, descriptor);
    else Reflect.deleteProperty(globalThis, name);
  }
});

/** Replace browser raster and HTTP boundaries while exercising image admission and sequencing. */
function fixture() {
  const bytes = new Uint8Array(33);
  bytes.set([137, 80, 78, 71, 13, 10, 26, 10, 0, 0, 0, 13, 73, 72, 68, 82]);
  const view = new DataView(bytes.buffer);
  view.setUint32(16, 768);
  view.setUint32(20, 512);
  const blob = new Blob([bytes], { type: 'image/png' });
  Object.defineProperty(blob, 'arrayBuffer', { value: async () => bytes.buffer });
  const normalized = new Uint8Array(bytes);
  normalized[24] = 8;
  normalized[25] = 6;
  const encoded = new Blob([normalized], { type: 'image/png' });
  Object.defineProperty(encoded, 'arrayBuffer', { value: async () => normalized.buffer });
  const close = jest.fn<() => void>();
  const bitmap = { width: 768, height: 512, close };
  const decode = jest.fn<() => Promise<typeof bitmap>>().mockResolvedValue(bitmap);
  const draw = jest.fn<() => void>();
  const encode = jest.fn<() => Promise<Blob>>().mockResolvedValue(encoded);
  const canvases: RasterCanvas[] = [];
  const sizes: [number, number][] = [];
  /** Model only the native raster allocation and encoding boundary. */
  class RasterCanvas {
    /** Retain allocation dimensions independently of subsequent disposal. */
    constructor(
      public width: number,
      public height: number,
    ) {
      sizes.push([width, height]);
      canvases.push(this);
    }
    /** Expose the drawing primitive used for sRGB normalization. */
    getContext() {
      return { drawImage: draw };
    }
    convertToBlob = encode;
  }
  Object.defineProperty(globalThis, 'createImageBitmap', { configurable: true, value: decode });
  Object.defineProperty(globalThis, 'OffscreenCanvas', { configurable: true, value: RasterCanvas });
  // Provide the HTTP response members consumed by the existing media boundary.
  const response = {
    ok: true,
    blob: async () => blob,
    headers: { get: (_name: string) => 'generated.png' },
  } as Response;
  const fetch = jest.fn<typeof globalThis.fetch>().mockResolvedValue(response);
  const images = new BrowserCanvasGenerationImages('http://127.0.0.1:8189', fetch);
  return { bytes, blob, fetch, images, normalized, bitmap, decode, draw, encode, canvases, sizes };
}

test('normalizes the full-resolution output without deriving pixels from a viewer', async () => {
  const f = fixture();
  const file = { filename: 'generated.png', subfolder: '', type: 'temp' as const };
  const image = await f.images.load(file, new CanvasCancellation(), null);
  expect(decodeCanvasImage(image!.image)).toEqual(f.normalized);
  expect(f.decode).toHaveBeenCalledWith(f.blob);
  expect(f.sizes).toEqual([[768, 512]]);
  expect(f.draw).toHaveBeenCalledWith(f.bitmap, 0, 0);
  expect(f.encode).toHaveBeenCalledWith({ type: 'image/png' });
  expect(f.bitmap.close).toHaveBeenCalledTimes(1);
  expect(f.canvases.map(({ width, height }) => [width, height])).toEqual([[1, 1]]);
  expect(f.fetch.mock.calls[0]?.[0]).toBe('/sugarcubes/output-saving/image');
  expect(f.fetch.mock.calls[0]?.[1]?.body).toBe(JSON.stringify({ file, format: 'png' }));
});

test('propagates cancellation into an in-flight full-resolution download', async () => {
  const f = fixture();
  const cancellation = new CanvasCancellation();
  f.fetch.mockImplementation(async (_input, init) => {
    cancellation.cancel();
    expect(init?.signal?.aborted).toBe(true);
    throw new DOMException('Cancelled', 'AbortError');
  });
  await expect(
    f.images.load({ filename: 'generated.png', subfolder: '', type: 'temp' }, cancellation, null),
  ).rejects.toThrow('Cancelled');
});

test('rejects a rendition exceeding the native transfer budget before decoding or insertion', async () => {
  const f = fixture();
  Object.defineProperty(f.blob, 'size', { value: 64 * 1024 * 1024 + 1 });
  await expect(
    f.images.load(
      { filename: 'generated.png', subfolder: '', type: 'temp' },
      new CanvasCancellation(),
      null,
    ),
  ).rejects.toThrow('byte budget');
  expect(f.decode).not.toHaveBeenCalled();
});

test('rejects oversized declared geometry before allocating browser pixels', async () => {
  const f = fixture();
  const header = new DataView(f.bytes.buffer);
  header.setUint32(16, 16384);
  header.setUint32(20, 16384);
  await expect(
    f.images.load(
      { filename: 'generated.png', subfolder: '', type: 'temp' },
      new CanvasCancellation(),
      null,
    ),
  ).rejects.toThrow();
  expect(f.decode).not.toHaveBeenCalled();
  expect(f.canvases).toHaveLength(0);
});

test('releases decoded pixels when cancelled before normalization', async () => {
  const f = fixture();
  const cancellation = new CanvasCancellation();
  f.decode.mockImplementation(async () => {
    cancellation.cancel();
    return f.bitmap;
  });
  await expect(
    f.images.load({ filename: 'generated.png', subfolder: '', type: 'temp' }, cancellation, null),
  ).resolves.toBeNull();
  expect(f.bitmap.close).toHaveBeenCalledTimes(1);
  expect(f.canvases).toHaveLength(0);
});

test('releases browser allocations if pixel normalization fails', async () => {
  const f = fixture();
  f.draw.mockImplementation(() => {
    throw new Error('Raster conversion failed.');
  });
  await expect(
    f.images.load(
      { filename: 'generated.png', subfolder: '', type: 'temp' },
      new CanvasCancellation(),
      null,
    ),
  ).rejects.toThrow('Raster conversion failed.');
  expect(f.bitmap.close).toHaveBeenCalledTimes(1);
  expect(f.canvases.map(({ width, height }) => [width, height])).toEqual([[1, 1]]);
});
