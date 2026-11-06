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
/** Prove typed artifact transport and closed response validation independently of rendering. */
import { jest } from '@jest/globals';
import type { SessionOutput } from '../../../integrations/contracts/SessionProtocol.generated.js';
import {
  OutputArtifactClient,
  decodeArtifactResult,
} from '../../../frontend/comfyui/ui/output_saving/OutputArtifactClient.js';

const file = { filename: 'final.png', subfolder: '', type: 'output' as const };
const receipt = {
  file: {
    storage: 'output' as const,
    filename: file.filename,
    subfolder: '',
    managedArtifact: null,
  },
  digest: '0'.repeat(64),
  bytes: 70,
  width: 4,
  height: 3,
  media: 'image' as const,
  mime: 'image/png',
  metadataEmbedded: false,
  animated: false,
};

/** Encode the real streaming response interface consumed by the bounded client. */
function response(value: unknown): Response {
  return new Response(JSON.stringify(value), {
    status: 200,
    headers: { 'Content-Type': 'application/json' },
  });
}

test('confirms exact bytes and projects artifact facts without forwarding workflow authority', async () => {
  const fetch = jest
    .fn<typeof globalThis.fetch>()
    .mockResolvedValue(response({ available: true, receipt }));
  const client = new OutputArtifactClient(fetch);
  expect(await client.receipt(file)).toEqual({ available: true, receipt });
  expect(fetch.mock.calls[0]?.[0]).toBe('/sugarcubes/output-saving/artifact-receipt');
  expect(JSON.parse(String(fetch.mock.calls[0]?.[1]?.body))).toEqual({ file });
  fetch.mockResolvedValueOnce(response({ available: true, receipt }));
  const output: SessionOutput = {
    ...receipt,
    id: 'output',
    entryId: 'entry',
    backend: 'correlation',
    promptId: null,
    runId: 'run',
    runOrder: 0,
    sourceId: 'source',
    scene: null,
    listIndex: 0,
    batchIndex: 0,
    imageIndex: 0,
    assetDigest: null,
  };
  expect(await client.resolve(output)).toEqual({ available: true, receipt });
  expect(JSON.parse(String(fetch.mock.calls[1]?.[1]?.body))).toEqual({
    expected: {
      file: receipt.file,
      digest: receipt.digest,
      bytes: receipt.bytes,
      width: receipt.width,
      height: receipt.height,
      media: receipt.media,
      mime: receipt.mime,
      metadataEmbedded: receipt.metadataEmbedded,
      promptId: null,
    },
  });
});

test.each(['missing', 'changed', 'unsupported', 'inaccessible', 'budget', 'cancelled'])(
  'keeps %s as an individual unavailable result',
  async (code) => {
    const value = { available: false, code, message: 'This recorded output is unavailable.' };
    const fetch = jest.fn<typeof globalThis.fetch>().mockResolvedValue(response(value));
    expect(await new OutputArtifactClient(fetch).receipt(file)).toEqual(value);
  },
);

test.each([
  { ...receipt, digest: 'A'.repeat(64) },
  { ...receipt, bytes: 0 },
  { ...receipt, bytes: 8589934593 },
  { ...receipt, width: Number.NaN },
  { ...receipt, metadataEmbedded: 0 },
  { ...receipt, file: { ...receipt.file, filename: '../outside.png' } },
  { ...receipt, file: { ...receipt.file, managedArtifact: 'arbitrary' } },
  { ...receipt, file: { ...receipt.file, subfolder: '__sugarcubes_managed__/unregistered' } },
  { ...receipt, privatePath: 'C:/private/file.png' },
])('rejects malformed identity facts and unknown authority fields', (invalid) => {
  expect(() => decodeArtifactResult({ available: true, receipt: invalid })).toThrow(
    'Invalid output artifact response',
  );
});

test('bounds streamed response allocation', async () => {
  const fetch = jest
    .fn<typeof globalThis.fetch>()
    .mockResolvedValue(new Response('x'.repeat(16385)));
  await expect(new OutputArtifactClient(fetch).receipt(file)).rejects.toThrow('byte limit');
});

test('propagates already cancelled caller and removes owned timer/listener', async () => {
  const controller = new AbortController();
  controller.abort();
  const fetch = jest.fn<typeof globalThis.fetch>().mockImplementation(async (_url, options) => {
    expect(options?.signal?.aborted).toBe(true);
    throw new DOMException('Aborted', 'AbortError');
  });
  const remove = jest.spyOn(controller.signal, 'removeEventListener');
  await expect(new OutputArtifactClient(fetch).receipt(file, controller.signal)).rejects.toThrow(
    'Aborted',
  );
  expect(remove).toHaveBeenCalledWith('abort', expect.any(Function));
});
test('rejects a resolver response with changed bytes despite a valid closed shape', async () => {
  const fetch = jest
    .fn<typeof globalThis.fetch>()
    .mockResolvedValue(
      response({ available: true, receipt: { ...receipt, digest: '1'.repeat(64) } }),
    );
  const output: SessionOutput = {
    ...receipt,
    id: 'output',
    entryId: 'entry',
    backend: 'correlation',
    promptId: null,
    runId: 'run',
    runOrder: 0,
    sourceId: 'source',
    scene: null,
    listIndex: 0,
    batchIndex: 0,
    imageIndex: 0,
    assetDigest: null,
  };
  await expect(new OutputArtifactClient(fetch).resolve(output)).rejects.toThrow(
    'recorded identity',
  );
});

test('bounds request lifetime and releases its owned timeout after an abort', async () => {
  jest.useFakeTimers();
  try {
    const fetch = jest.fn<typeof globalThis.fetch>().mockImplementation(
      (_url, options) =>
        new Promise((_resolve, reject) => {
          options?.signal?.addEventListener(
            'abort',
            () => reject(new DOMException('Timed out', 'AbortError')),
            { once: true },
          );
        }),
    );
    const pending = new OutputArtifactClient(fetch).receipt(file);
    const rejected = expect(pending).rejects.toThrow('Timed out');
    await jest.advanceTimersByTimeAsync(20000);
    await rejected;
    expect(jest.getTimerCount()).toBe(0);
  } finally {
    jest.useRealTimers();
  }
});

test.each([
  ['animation.gif', 'image/gif', 'image', true],
  ['animation.webp', 'image/webp', 'image', true],
  ['animation.png', 'image/png', 'image', true],
  ['clip.mp4', 'video/mp4', 'video', false],
  ['clip.webm', 'video/webm', 'video', false],
])(
  'admits exact supported media facts for %s without widening native decoding',
  (filename, mime, media, animated) => {
    const value = { ...receipt, file: { ...receipt.file, filename }, mime, media, animated };
    expect(decodeArtifactResult({ available: true, receipt: value })).toEqual({
      available: true,
      receipt: value,
    });
  },
);
