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
  outputTransferUrl,
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
  return { ok: true, json: async () => value, blob: async () => blob } as Response;
}

test('requests shared policy and copies exact selected bytes without reconstructing preferences', async () => {
  const payload = { success: true, data: { file: jpeg, mime_type: 'image/jpeg' } };
  const fetch = jest.fn<typeof globalThis.fetch>().mockResolvedValue(response(payload));
  const write = jest
    .fn<(blob: Blob, type: 'image/png' | 'image/jpeg' | 'image/webp') => Promise<void>>()
    .mockResolvedValue(undefined);
  expect(
    await new BrowserOutputTransfer('http://localhost:8189', fetch, { write }).copy(
      file,
      () => true,
    ),
  ).toBe(true);
  expect(fetch.mock.calls[0]?.[1]?.body).toBe(JSON.stringify(file));
  expect(fetch.mock.calls[1]?.[0]).toBe(
    'http://localhost:8189/view?filename=output.jpg&subfolder=__sugarcubes_managed__%2Fidentity&type=output',
  );
  expect(write).toHaveBeenCalledWith(expect.any(Blob), 'image/jpeg');
});

test('refuses late bytes after a workflow switch or different selected output', async () => {
  let current = true;
  const fetch = jest
    .fn<typeof globalThis.fetch>()
    .mockResolvedValueOnce(
      response({ success: true, data: { file: jpeg, mime_type: 'image/jpeg' } }),
    )
    .mockImplementationOnce(async () => {
      current = false;
      return response({});
    });
  const write =
    jest.fn<(blob: Blob, type: 'image/png' | 'image/jpeg' | 'image/webp') => Promise<void>>();
  expect(
    await new BrowserOutputTransfer('http://localhost:8189', fetch, { write }).copy(
      file,
      () => current,
    ),
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
  expect(() => outputTransferUrl('file:///private', { file, mimeType: 'image/png' })).toThrow();
});
