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
/** Prove local output image reads enforce format, byte limits and cancellation. */
import { jest } from '@jest/globals';
import { readComfyOutputImage } from '../../../frontend/comfyui/ui/model_library/ComfyOutputImageDownload.js';
import type {
  ComfyOutputImageFetch,
  ComfyOutputImageResponse,
} from '../../../frontend/comfyui/ui/model_library/ComfyOutputImageDownload.js';

const url = new URL('/view?filename=test.png&type=output', document.baseURI).href;

/** Provide only the validated transport surface consumed by the image adapter. */
function response(type = 'image/png', size = 3): ComfyOutputImageResponse {
  let sent = false;
  return {
    ok: true,
    headers: { get: (name) => (name === 'content-type' ? type : null) },
    body: {
      getReader: () => ({
        read: async () => {
          if (sent) return { done: true, value: undefined };
          sent = true;
          return { done: false, value: new Uint8Array(size) };
        },
        cancel: async () => undefined,
      }),
    },
  };
}

test('preserves original image bytes and limits the request to the local Comfy endpoint', async () => {
  const request = jest.fn<ComfyOutputImageFetch>(async () => response());
  const image = await readComfyOutputImage(url, 'test.png', new AbortController().signal, request);
  expect([image.name, image.type, image.size]).toEqual(['test.png', 'image/png', 3]);
  expect(request).toHaveBeenCalledWith(
    url,
    expect.objectContaining({ credentials: 'same-origin', redirect: 'error' }),
  );
  await expect(
    readComfyOutputImage(
      'https://evil.test/view?type=output',
      'test.png',
      new AbortController().signal,
      request,
    ),
  ).rejects.toThrow('not a completed Comfy output');
  await expect(
    readComfyOutputImage('/view?type=input', 'test.png', new AbortController().signal, request),
  ).rejects.toThrow('not a completed Comfy output');
  expect(request).toHaveBeenCalledTimes(1);
});

test.each([
  ['text/html', 1, 'not a supported image'],
  ['image/svg+xml', 1, 'not a supported image'],
  ['image/png', 0, 'empty'],
  ['image/png', 20 * 1024 * 1024 + 1, '20 MiB'],
])('rejects format %s and actual decoded size %s', async (type, size, message) => {
  await expect(
    readComfyOutputImage(url, 'test.png', new AbortController().signal, async () =>
      response(type, size),
    ),
  ).rejects.toThrow(message);
});

test('propagates owner cancellation and times out stalled image requests', async () => {
  jest.useFakeTimers();
  const owner = new AbortController();
  const request: ComfyOutputImageFetch = async (_url, options) =>
    new Promise((_resolve, reject) => {
      options.signal?.addEventListener(
        'abort',
        () => {
          const reason: unknown = options.signal?.reason;
          reject(reason instanceof Error ? reason : new Error('Image request cancelled'));
        },
        {
          once: true,
        },
      );
    });
  try {
    const cancelled = readComfyOutputImage(url, 'test.png', owner.signal, request);
    const expected = expect(cancelled).rejects.toThrow('Stopped');
    owner.abort(new Error('Stopped'));
    await expected;
    const stalled = readComfyOutputImage(url, 'test.png', new AbortController().signal, request);
    const timeout = expect(stalled).rejects.toThrow('timed out');
    await jest.advanceTimersByTimeAsync(15_000);
    await timeout;
    expect(jest.getTimerCount()).toBe(0);
  } finally {
    jest.useRealTimers();
  }
});

test('does not start an image request whose owner has already cancelled', async () => {
  const owner = new AbortController();
  const request = jest.fn<ComfyOutputImageFetch>(async () => response());
  owner.abort(new Error('Image read stopped'));
  await expect(readComfyOutputImage(url, 'test.png', owner.signal, request)).rejects.toThrow(
    'Image read stopped',
  );
  expect(request).not.toHaveBeenCalled();
});

test.each(['response', 'body'] as const)(
  'rejects a cancelled image even when the %s completes after cancellation',
  async (stage) => {
    const owner = new AbortController();
    const cancel = jest.fn<() => Promise<void>>(async () => undefined);
    let sent = false;
    const imageResponse: ComfyOutputImageResponse = {
      ...response(),
      body: {
        getReader: () => ({
          read: async () => {
            if (sent) return { done: true, value: undefined };
            sent = true;
            if (stage === 'body') owner.abort(new Error('Image read stopped'));
            return { done: false, value: new Uint8Array([1]) };
          },
          cancel,
        }),
      },
    };
    const request: ComfyOutputImageFetch = async () => {
      if (stage === 'response') owner.abort(new Error('Image read stopped'));
      return imageResponse;
    };
    await expect(readComfyOutputImage(url, 'test.png', owner.signal, request)).rejects.toThrow(
      'Image read stopped',
    );
    if (stage === 'body') expect(cancel).toHaveBeenCalledTimes(1);
  },
);
