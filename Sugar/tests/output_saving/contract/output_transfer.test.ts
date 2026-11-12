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
/** Prove transfer transport validation and stale-output fencing without browser preference copies. */
import { jest } from '@jest/globals';
import {
  OutputTransferClient,
  outputFileUrl,
} from '../../../frontend/comfyui/ui/output_saving/OutputTransferClient.js';
import { BrowserOutputTransfer } from '../../../frontend/comfyui/ui/output_saving/BrowserOutputTransfer.js';

const file = {
  filename: 'output.png',
  subfolder: '__sugarcubes_managed__/identity',
  type: 'output' as const,
};
const jpeg = { ...file, filename: 'output.jpg' };

/** Provide only the external HTTP response fields consumed by these adapters. */
function response(
  value: unknown,
  blob = new Blob(['image bytes'], { type: 'image/jpeg' }),
): Response {
  return {
    ok: true,
    json: async () => value,
    blob: async () => blob,
    headers: { get: (name: string) => (name === 'X-SugarCubes-Filename' ? 'output.jpg' : null) },
  } as Response;
}

test('requests shared policy and copies exact selected bytes without reconstructing preferences', async () => {
  const payload = { success: true, data: { file: jpeg, mime_type: 'image/jpeg' } };
  const fetch = jest.fn<typeof globalThis.fetch>().mockResolvedValue(response(payload));
  const write = jest
    .fn<(blob: Blob, type: 'image/png' | 'image/jpeg' | 'image/webp') => Promise<void>>()
    .mockResolvedValue(undefined);
  expect(
    await new BrowserOutputTransfer(
      'http://localhost:8189',
      fetch,
      { write },
      { start: jest.fn() },
    ).copy(file, () => true, 'preferred'),
  ).toBe(true);
  expect(fetch.mock.calls[0]?.[1]?.body).toBe(JSON.stringify({ file, format: 'preferred' }));
  expect(fetch.mock.calls[0]?.[0]).toBe('/sugarcubes/output-saving/image');
  expect(write).toHaveBeenCalledWith(expect.any(Blob), 'image/jpeg');
});

test('prepares the preferred exact file and full-fidelity URL for synchronous outbound dragging', async () => {
  const payload = { success: true, data: { file: jpeg, mime_type: 'image/jpeg' } };
  const fetch = jest.fn<typeof globalThis.fetch>().mockResolvedValue(response(payload));
  const write =
    jest.fn<(blob: Blob, type: 'image/png' | 'image/jpeg' | 'image/webp') => Promise<void>>();
  const transfer = new BrowserOutputTransfer(
    'http://localhost:8189',
    fetch,
    { write },
    { start: jest.fn() },
  );
  const result = await transfer.drag(file, new AbortController().signal);
  expect(result?.file.name).toBe('output.jpg');
  expect(result?.file.type).toBe('image/jpeg');
  expect(result?.file.size).toBe(new Blob(['image bytes']).size);
  expect(result?.url).toBe(
    'http://localhost:8189/view?filename=output.jpg&subfolder=__sugarcubes_managed__%2Fidentity&type=output',
  );
  expect(write).not.toHaveBeenCalled();
  const controller = new AbortController();
  controller.abort();
  expect(await transfer.drag(file, controller.signal)).toBeNull();
  expect(fetch).toHaveBeenCalledTimes(2);
});

test('propagates drag cancellation to finite HTTP work and rejects late bytes', async () => {
  const controller = new AbortController();
  const fetch = jest
    .fn<typeof globalThis.fetch>()
    .mockResolvedValueOnce(
      response({ success: true, data: { file: jpeg, mime_type: 'image/jpeg' } }),
    )
    .mockImplementationOnce(async (_url, request) => {
      controller.abort();
      expect(request?.signal?.aborted).toBe(true);
      return response({});
    });
  const write =
    jest.fn<(blob: Blob, type: 'image/png' | 'image/jpeg' | 'image/webp') => Promise<void>>();
  expect(
    await new BrowserOutputTransfer(
      'http://localhost:8189',
      fetch,
      { write },
      { start: jest.fn() },
    ).drag(file, controller.signal),
  ).toBeNull();
});

test.each(['image/png', 'image/jpeg'] as const)(
  'attaches every original %s byte for each batch item independently of display thumbnails',
  async (mimeType) => {
    const write =
      jest.fn<(blob: Blob, type: 'image/png' | 'image/jpeg' | 'image/webp') => Promise<void>>();
    for (const batchIndex of [0, 1, 2]) {
      const selected = {
        ...file,
        filename: `batch-${batchIndex}.${mimeType === 'image/png' ? 'png' : 'jpg'}`,
      };
      const bytes = Uint8Array.from({ length: 8192 }, (_, index) => (index + batchIndex) % 256);
      const fetch = jest
        .fn<typeof globalThis.fetch>()
        .mockResolvedValue(
          response(
            { success: true, data: { file: selected, mime_type: mimeType } },
            new Blob([bytes], { type: mimeType }),
          ),
        );
      const result = await new BrowserOutputTransfer(
        'http://localhost:8189',
        fetch,
        {
          write,
        },
        { start: jest.fn() },
      ).drag(selected, new AbortController().signal);
      expect(result?.file.name).toBe(selected.filename);
      expect(result?.file.type).toBe(mimeType);
      expect(result?.file.size).toBe(bytes.length);
      if (!result) throw new Error('Missing exact batch transfer.');
      const attached = await new Promise<ArrayBuffer>((resolve, reject) => {
        const reader = new FileReader();
        reader.onload = () =>
          reader.result instanceof ArrayBuffer
            ? resolve(reader.result)
            : reject(new Error('Missing binary drag data.'));
        reader.onerror = () => reject(reader.error ?? new Error('Drag data could not be read.'));
        reader.readAsArrayBuffer(result.file);
      });
      expect(new Uint8Array(attached)).toEqual(bytes);
      expect(new URL(result.url).searchParams.has('preview')).toBe(false);
    }
    expect(write).not.toHaveBeenCalled();
  },
);

test('refuses late bytes after a workflow switch or different selected output', async () => {
  let current = true;
  const fetch = jest.fn<typeof globalThis.fetch>().mockImplementationOnce(async () => {
    current = false;
    return response({});
  });
  const write =
    jest.fn<(blob: Blob, type: 'image/png' | 'image/jpeg' | 'image/webp') => Promise<void>>();
  expect(
    await new BrowserOutputTransfer(
      'http://localhost:8189',
      fetch,
      { write },
      { start: jest.fn() },
    ).copy(file, () => current, 'jpeg'),
  ).toBe(false);
  expect(write).not.toHaveBeenCalled();
});

test.each([
  {},
  { file: { ...file, filename: '../secret.png' }, mime_type: 'image/png' },
  { file: { ...file, subfolder: 'C:/secret' }, mime_type: 'image/png' },
  { file: { ...file, subfolder: '../secret' }, mime_type: 'image/png' },
  { file, mime_type: 'text/html' },
  { file: { ...file, type: 'other' }, mime_type: 'image/png' },
])('rejects malformed portable selection: %j', async (data) => {
  const fetch = jest
    .fn<typeof globalThis.fetch>()
    .mockResolvedValue(response({ success: true, data }));
  await expect(new OutputTransferClient(fetch).choose(file)).rejects.toThrow(
    'Invalid output transfer response',
  );
});

test('preserves literal server errors and rejects non-HTTP image hosts', async () => {
  const fetch = jest
    .fn<typeof globalThis.fetch>()
    .mockResolvedValue(response({ success: false, error: { message: '<unavailable>' } }));
  await expect(new OutputTransferClient(fetch).choose(file)).rejects.toThrow('<unavailable>');
  expect(() => outputFileUrl('file:///private', file)).toThrow();
});
