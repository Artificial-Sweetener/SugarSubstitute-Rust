/** @jest-environment node */
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
/** Prove retained actions authenticate exact scoped originals without querying missing backend files. */
import { jest } from '@jest/globals';
import { createHash, webcrypto } from 'node:crypto';
import type { SessionOutput } from '../../../integrations/contracts/SessionProtocol.generated.js';
import { BrowserOutputTransfer } from '../../../frontend/comfyui/ui/output_saving/BrowserOutputTransfer.js';
import {
  BrowserRetainedOutputTransfer,
  retainedOutputCapabilities,
  type BrowserRetainedOutputSource,
} from '../../../frontend/comfyui/ui/output_saving/BrowserRetainedOutputTransfer.js';
import type { OutputImageRendition } from '../../../frontend/comfyui/ui/output_saving/BrowserImageRendition.js';

const origin = 'http://127.0.0.1:8188';
const png = new Uint8Array([
  137, 80, 78, 71, 13, 10, 26, 10, 0, 0, 0, 13, 73, 72, 68, 82, 0, 0, 0, 1, 0, 0, 0, 1, 8, 6, 0, 0,
  0, 31, 21, 196, 137, 0, 0, 0, 13, 73, 68, 65, 84, 120, 156, 99, 248, 207, 192, 240, 31, 0, 5, 0,
  1, 255, 137, 153, 61, 29, 0, 0, 0, 0, 73, 69, 78, 68, 174, 66, 96, 130,
]);
const file = { filename: 'previous.png', subfolder: '', type: 'temp' as const };
const digest = createHash('sha256').update(png).digest('hex');
const output: SessionOutput = {
  id: '11111111-1111-4111-8111-111111111111',
  entryId: '22222222-2222-4222-8222-222222222222',
  backend: '33333333-3333-4333-8333-333333333333',
  promptId: null,
  runId: '44444444-4444-4444-8444-444444444444',
  runOrder: 0,
  sourceId: '55555555-5555-4555-8555-555555555555',
  scene: null,
  listIndex: 0,
  batchIndex: 0,
  imageIndex: 0,
  file: { storage: 'temp', filename: file.filename, subfolder: '', managedArtifact: null },
  assetDigest: digest,
  digest,
  bytes: png.length,
  width: 1,
  height: 1,
  media: 'image',
  mime: 'image/png',
  metadataEmbedded: false,
};
const binding = {
  entryId: output.entryId,
  runtimeSession: 'runtime-a',
  generation: 1,
  graphRevision: 1,
  sourceRevision: 1,
  navigationRevision: 1,
};
const url = `${origin}/__sugarsubstitute/session-media/v1/${'a'.repeat(32)}/${output.id}?binding=${encodeURIComponent(JSON.stringify(binding))}`;
const source: BrowserRetainedOutputSource = { output, binding, url };
const cryptoDescriptor = Object.getOwnPropertyDescriptor(globalThis, 'crypto');
beforeAll(() => {
  Object.defineProperty(globalThis, 'crypto', { configurable: true, value: webcrypto });
});
afterAll(() => {
  if (cryptoDescriptor) Object.defineProperty(globalThis, 'crypto', cryptoDescriptor);
  else Reflect.deleteProperty(globalThis, 'crypto');
});

/** Return truthful complete original metadata with optional exact body. */
function original(body: Uint8Array<ArrayBuffer> | null = png): Response {
  return new Response(body, {
    status: 200,
    headers: { 'Content-Type': 'image/png', 'Content-Length': String(png.length) },
  });
}

/** Keep the existing backend policy owner while refusing all file descriptor requests. */
function transport() {
  return jest.fn<typeof globalThis.fetch>().mockImplementation(async (input, init) => {
    if (String(input).endsWith('/media-profile'))
      return new Response(
        JSON.stringify({
          success: true,
          data: { preferred_copy_format: 'jpeg', jpeg_quality: 73, reveal: true, photoshop: true },
        }),
      );
    if (String(input) !== url) throw new Error('A missing backend file endpoint was queried.');
    return original(init?.method === 'HEAD' ? null : png);
  });
}

test('reads retained PNG copy/drag/download through exact scoped bytes and current copy policy', async () => {
  const fetch = transport();
  const write = jest
    .fn<(blob: Blob, mime: 'image/png' | 'image/jpeg' | 'image/webp') => Promise<void>>()
    .mockResolvedValue(undefined);
  const start = jest.fn<(url: string, filename?: string) => void>();
  const encode = jest
    .fn<OutputImageRendition['encode']>()
    .mockResolvedValue(new Blob(['encoded jpeg'], { type: 'image/jpeg' }));
  const transfer = new BrowserOutputTransfer(origin, fetch, { write }, { start }, { encode });
  await transfer.refresh();
  expect(transfer.profile().photoshop).toBe(true);
  expect(transfer.profile(source)).toMatchObject({ fileAction: 'download', photoshop: false });
  expect(await transfer.copy(file, () => true, 'preferred', undefined, source)).toBe(true);
  expect(encode).toHaveBeenCalledWith(
    expect.any(Blob),
    'image/jpeg',
    0.73,
    { width: 1, height: 1 },
    undefined,
  );
  expect(write).toHaveBeenCalledWith(expect.any(Blob), 'image/jpeg');
  expect(await transfer.copy(file, () => true, 'png', undefined, source)).toBe(true);
  expect(write).toHaveBeenLastCalledWith(expect.any(Blob), 'image/png');
  const dragged = await transfer.drag(file, new AbortController().signal, source);
  expect(dragged?.file.name).toBe(file.filename);
  expect(new Uint8Array(await dragged!.file.arrayBuffer())).toEqual(png);
  expect(await transfer.fileAction(file, () => true, undefined, 'image', source)).toBe(true);
  expect(start).toHaveBeenCalledWith(url, file.filename);
  expect(
    await transfer.photoshop('restored', [{ file, name: 'result', retained: source }], () => true),
  ).toBe(false);
  expect(
    fetch.mock.calls.every(
      ([input]) => String(input) === url || String(input).endsWith('/media-profile'),
    ),
  ).toBe(true);
  transfer.dispose();
});

test.each([
  { ...source, url: url.replace(origin, 'https://foreign.example') },
  { ...source, url: url.replace(output.id, '66666666-6666-4666-8666-666666666666') },
  { ...source, url: url + '&path=C:/private' },
  { ...source, url: url + '#fragment' },
  { ...source, binding: { ...binding, generation: 2 } },
  { ...source, output: { ...output, assetDigest: null } },
])('rejects foreign, stale and malformed capabilities before HTTP', async (invalid) => {
  const fetch = transport();
  await expect(
    new BrowserRetainedOutputTransfer(origin, fetch).image(file, invalid),
  ).rejects.toThrow();
  expect(fetch).not.toHaveBeenCalled();
});

test('rejects changed body bytes even when the host metadata names the expected output', async () => {
  const changed = png.slice();
  changed[45] = (changed[45] ?? 0) ^ 1;
  const fetch = jest.fn<typeof globalThis.fetch>().mockResolvedValue(original(changed));
  await expect(
    new BrowserRetainedOutputTransfer(origin, fetch).image(file, source),
  ).rejects.toThrow('confirmed identity');
});

test('never offers a direct partial download or image clipboard for a large video', async () => {
  const large = {
    ...source,
    output: { ...output, bytes: 64 * 1024 * 1024 + 1, media: 'video' as const, mime: 'video/mp4' },
  };
  expect(retainedOutputCapabilities(large)).toEqual({
    copy: false,
    drag: false,
    download: false,
    reveal: false,
    photoshop: false,
  });
  const fetch = transport();
  const transfer = new BrowserOutputTransfer(
    origin,
    fetch,
    {
      write:
        jest.fn<(blob: Blob, mime: 'image/png' | 'image/jpeg' | 'image/webp') => Promise<void>>(),
    },
    { start: jest.fn() },
  );
  expect(await transfer.fileAction(file, () => true, undefined, 'media', large)).toBe(false);
  expect(await transfer.copy(file, () => true, 'png', undefined, large)).toBe(false);
  expect(fetch).not.toHaveBeenCalled();
  transfer.dispose();
});

test('refuses partial framing and does not publish after selection retirement', async () => {
  const fetch = jest.fn<typeof globalThis.fetch>().mockResolvedValue(
    new Response(png, {
      status: 206,
      headers: {
        'Content-Type': 'image/png',
        'Content-Length': String(png.length),
        'Content-Range': `bytes 0-${png.length - 1}/${png.length}`,
      },
    }),
  );
  await expect(
    new BrowserRetainedOutputTransfer(origin, fetch).image(file, source),
  ).rejects.toThrow('confirmed original');
  let current = true;
  const later = transport().mockImplementation(async (input) => {
    current = false;
    return String(input).endsWith('/media-profile') ? new Response('{}') : original();
  });
  const write =
    jest.fn<(blob: Blob, mime: 'image/png' | 'image/jpeg' | 'image/webp') => Promise<void>>();
  const transfer = new BrowserOutputTransfer(origin, later, { write }, { start: jest.fn() });
  expect(await transfer.copy(file, () => current, 'png', undefined, source)).toBe(false);
  expect(write).not.toHaveBeenCalled();
  transfer.dispose();
});
