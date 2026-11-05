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
/** Prove local capability gating, explicit copy formats and portable remote downloads. */
import { jest } from '@jest/globals';
import { BrowserOutputTransfer } from '../../../frontend/comfyui/ui/output_saving/BrowserOutputTransfer.js';
import { OutputMediaClient } from '../../../frontend/comfyui/ui/output_saving/OutputMediaClient.js';

const file = { filename: 'image.png', subfolder: 'outputs', type: 'output' as const };

/** Supply only HTTP response fields consumed by the validated browser adapter. */
function response(value: unknown, type = 'image/jpeg', filename = 'image.jpg'): Response {
  return {
    ok: true,
    json: async () => value,
    blob: async () => new Blob(['full-resolution'], { type }),
    headers: {
      get: (name: string) =>
        name === 'X-SugarCubes-Filename' ? encodeURIComponent(filename) : null,
    },
  } as Response;
}

test.each(['http://127.0.0.1:8189', 'http://localhost:8189', 'http://[::1]:8189'])(
  'offers reveal only with a server-qualified local connection: %s',
  async (origin) => {
    const fetch = jest.fn<typeof globalThis.fetch>().mockResolvedValue(
      response({
        success: true,
        data: { preferred_copy_format: 'png', reveal: true, photoshop: true },
      }),
    );
    expect(await new OutputMediaClient(origin, fetch).profile()).toEqual({
      preferredCopyFormat: 'png',
      fileAction: 'reveal',
      photoshop: true,
    });
  },
);

test.each([
  'https://comfy.example',
  'http://192.168.1.5:8189',
  'https://localhost.attacker.example',
])('refuses a server reveal advertisement for a remote browser origin: %s', async (origin) => {
  const fetch = jest.fn<typeof globalThis.fetch>().mockResolvedValue(
    response({
      success: true,
      data: { preferred_copy_format: 'jpeg', reveal: true, photoshop: true },
    }),
  );
  expect(await new OutputMediaClient(origin, fetch).profile()).toEqual({
    preferredCopyFormat: 'jpeg',
    fileAction: 'download',
    photoshop: false,
  });
});

test('uses explicit copy formats and the preferred download endpoint without navigating another tab', async () => {
  const fetch = jest.fn<typeof globalThis.fetch>().mockResolvedValue(response({}));
  const write = jest
    .fn<(blob: Blob, mime: 'image/png' | 'image/jpeg' | 'image/webp') => Promise<void>>()
    .mockResolvedValue(undefined);
  const start = jest.fn<(url: string) => void>();
  const transfer = new BrowserOutputTransfer('https://comfy.example', fetch, { write }, { start });
  expect(await transfer.copy(file, () => true, 'jpeg')).toBe(true);
  expect(fetch.mock.calls[0]?.[1]?.body).toBe(JSON.stringify({ file, format: 'jpeg' }));
  expect(write).toHaveBeenCalledWith(expect.any(Blob), 'image/jpeg');
  expect(await transfer.fileAction(file, () => true)).toBe(true);
  const url = new URL(start.mock.calls[0]![0]);
  expect(url.pathname).toBe('/sugarcubes/output-saving/image');
  expect(url.searchParams.get('format')).toBe('preferred');
  expect(url.searchParams.get('filename')).toBe('image.png');
  const clip = { filename: 'clip.mp4', subfolder: 'outputs', type: 'output' as const };
  expect(await transfer.fileAction(clip, () => true, undefined, 'media')).toBe(true);
  const original = new URL(start.mock.calls[1]![0]);
  expect(original.origin + original.pathname).toBe('https://comfy.example/view');
  expect(Object.fromEntries(original.searchParams)).toEqual(clip);
  expect(fetch).toHaveBeenCalledTimes(1);
  transfer.dispose();
});

test('refreshes preference metadata and routes reveal without a download fallback on success', async () => {
  const fetch = jest
    .fn<typeof globalThis.fetch>()
    .mockResolvedValueOnce(
      response({
        success: true,
        data: { preferred_copy_format: 'jpeg', reveal: true, photoshop: false },
      }),
    )
    .mockResolvedValueOnce(response({ success: true }));
  const start = jest.fn();
  const transfer = new BrowserOutputTransfer(
    'http://127.0.0.1:8189',
    fetch,
    { write: async () => undefined },
    { start },
  );
  const listener = jest.fn();
  transfer.subscribe(listener);
  await transfer.refresh();
  expect(transfer.profile()).toEqual({
    preferredCopyFormat: 'jpeg',
    fileAction: 'reveal',
    photoshop: false,
  });
  // Photoshop stays refused while the server does not offer it.
  expect(await transfer.photoshop('image', [{ file, name: 'Save 1' }], () => true)).toBe(false);
  expect(listener).toHaveBeenCalledTimes(1);
  expect(await transfer.fileAction(file, () => true)).toBe(true);
  expect(fetch.mock.calls[1]?.[0]).toBe('/sugarcubes/output-saving/reveal');
  expect(fetch.mock.calls[1]?.[1]?.body).toBe(JSON.stringify(file));
  expect(start).not.toHaveBeenCalled();
  transfer.dispose();
  expect(await transfer.copy(file, () => true, 'png')).toBe(false);
});

test.each([
  { success: true, data: { preferred_copy_format: 'gif', reveal: true } },
  { success: true, data: { preferred_copy_format: 'png', reveal: 'yes', photoshop: false } },
  { success: true, data: { preferred_copy_format: 'png', reveal: true } },
  { success: false },
])('rejects malformed media policy: %j', async (value) => {
  const fetch = jest.fn<typeof globalThis.fetch>().mockResolvedValue(response(value));
  await expect(new OutputMediaClient('http://localhost:8189', fetch).profile()).rejects.toThrow(
    'capabilities',
  );
});

test('opens images in Photoshop through the local route with each layer name', async () => {
  const fetch = jest
    .fn<typeof globalThis.fetch>()
    .mockResolvedValueOnce(
      response({
        success: true,
        data: { preferred_copy_format: 'png', reveal: true, photoshop: true },
      }),
    )
    .mockResolvedValueOnce(response({ success: true }))
    .mockResolvedValueOnce(response({ success: false, error: { message: 'Photoshop is busy.' } }));
  const transfer = new BrowserOutputTransfer(
    'http://127.0.0.1:8189',
    fetch,
    { write: async () => undefined },
    { start: jest.fn() },
  );
  await transfer.refresh();
  const layers = [{ file, name: 'Save 1' }];
  expect(await transfer.photoshop('image', layers, () => false)).toBe(false);
  expect(await transfer.photoshop('image', layers, () => true)).toBe(true);
  expect(fetch.mock.calls[1]?.[0]).toBe('/sugarcubes/output-saving/photoshop');
  expect(fetch.mock.calls[1]?.[1]?.body).toBe(JSON.stringify({ document: 'image', layers }));
  await expect(transfer.photoshop('image', layers, () => true)).rejects.toThrow(
    'Photoshop is busy.',
  );
  transfer.dispose();
});

test('rejects unexpected image MIME types and unsafe returned filenames', async () => {
  for (const [type, filename] of [
    ['text/html', 'image.jpg'],
    ['image/jpeg', '../image.jpg'],
    ['image/png', 'bad.gif'],
  ]) {
    const fetch = jest
      .fn<typeof globalThis.fetch>()
      .mockResolvedValue(response({}, type, filename));
    await expect(
      new OutputMediaClient('http://localhost:8189', fetch).image(
        file,
        'jpeg',
        new AbortController().signal,
      ),
    ).rejects.toThrow('invalid');
  }
});
