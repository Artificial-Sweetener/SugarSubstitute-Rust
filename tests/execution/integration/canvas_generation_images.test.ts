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
/** Prove native generation transfer uses intrinsic image bytes and propagates lifecycle cancellation. */
import { jest } from '@jest/globals';
import { BrowserCanvasGenerationImages } from '../../../frontend/comfyui/ui/execution/infrastructure/BrowserCanvasGenerationImages.js';
import { CanvasCancellation } from '../../../integrations/shared/CanvasCancellation.js';
import { decodeCanvasImage } from '../../../integrations/shared/CanvasImageEncoding.js';

/** Replace only HTTP transport while exercising media validation and portable PNG encoding. */
function fixture() {
  const bytes = new Uint8Array(33);
  bytes.set([137, 80, 78, 71, 13, 10, 26, 10, 0, 0, 0, 13, 73, 72, 68, 82]);
  const view = new DataView(bytes.buffer);
  view.setUint32(16, 768);
  view.setUint32(20, 512);
  const blob = new Blob([bytes], { type: 'image/png' });
  Object.defineProperty(blob, 'arrayBuffer', { value: async () => bytes.buffer });
  // Provide the HTTP response members consumed by the existing media boundary.
  const response = {
    ok: true,
    blob: async () => blob,
    headers: { get: (_name: string) => 'generated.png' },
  } as Response;
  const fetch = jest.fn<typeof globalThis.fetch>().mockResolvedValue(response);
  const images = new BrowserCanvasGenerationImages('http://127.0.0.1:8189', fetch);
  return { bytes, blob, fetch, images };
}

test('transfers full-resolution PNG from the output service without deriving pixels from a viewer', async () => {
  const f = fixture();
  const file = { filename: 'generated.png', subfolder: '', type: 'temp' as const };
  const image = await f.images.load(file, new CanvasCancellation());
  expect(decodeCanvasImage(image)).toEqual(f.bytes);
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
    f.images.load({ filename: 'generated.png', subfolder: '', type: 'temp' }, cancellation),
  ).rejects.toThrow('Cancelled');
});

test('rejects a rendition exceeding the native transfer budget before decoding or insertion', async () => {
  const f = fixture();
  Object.defineProperty(f.blob, 'size', { value: 64 * 1024 * 1024 + 1 });
  await expect(
    f.images.load(
      { filename: 'generated.png', subfolder: '', type: 'temp' },
      new CanvasCancellation(),
    ),
  ).rejects.toThrow('byte budget');
});
