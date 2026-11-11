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
/** Prove installed-asset transport classification without disguising missing runtime contracts. */
import { afterEach, expect, jest, test } from '@jest/globals';
import {
  fetchComfyRuntimeText,
  isComfyModuleTransportFailure,
  ComfyRuntimeAssetUnavailableError,
  type ComfyRuntimeAssetRequester,
} from '../../../frontend/comfyui/ui/surface/ComfyRuntimeAssetRequest.js';

afterEach(() => {
  jest.useRealTimers();
});

/** Keep the original failure and URL available to connection recovery diagnostics. */
test('classifies a failed network request and clears its timeout', async () => {
  jest.useFakeTimers();
  const cause = new TypeError('Failed to fetch');
  const request = jest.fn<ComfyRuntimeAssetRequester>().mockRejectedValue(cause);
  await expect(fetchComfyRuntimeText('/assets/GraphView.js', request)).rejects.toMatchObject({
    name: 'ComfyRuntimeAssetUnavailableError',
    url: '/assets/GraphView.js',
    cause,
  });
  expect(jest.getTimerCount()).toBe(0);
});

/** Retry temporary server responses while keeping missing assets terminal and actionable. */
test.each([408, 429, 500, 503])('classifies HTTP %s as temporary', async (status) => {
  const request = jest
    .fn<ComfyRuntimeAssetRequester>()
    .mockResolvedValue({ ok: false, status, text: async () => '' });
  await expect(fetchComfyRuntimeText('/assets/GraphView.js', request)).rejects.toBeInstanceOf(
    ComfyRuntimeAssetUnavailableError,
  );
});
test('retains a missing installed asset as a terminal compatibility failure', async () => {
  const request = jest
    .fn<ComfyRuntimeAssetRequester>()
    .mockResolvedValue({ ok: false, status: 404, text: async () => '' });
  await expect(fetchComfyRuntimeText('/assets/GraphView.js', request)).rejects.toThrow(
    'status 404',
  );
  await expect(fetchComfyRuntimeText('/assets/GraphView.js', request)).rejects.not.toBeInstanceOf(
    ComfyRuntimeAssetUnavailableError,
  );
});

/** Abort a stalled installed-asset request after the bounded connection timeout. */
test('aborts a stalled request and retains timeout transport context', async () => {
  jest.useFakeTimers();
  const request: ComfyRuntimeAssetRequester = async (_url, options) =>
    new Promise((_resolve, reject) => {
      options.signal.addEventListener(
        'abort',
        () => reject(new DOMException('Timed out', 'AbortError')),
        { once: true },
      );
    });
  const pending = fetchComfyRuntimeText('/assets/GraphView.js', request);
  const proof = expect(pending).rejects.toBeInstanceOf(ComfyRuntimeAssetUnavailableError);
  await jest.advanceTimersByTimeAsync(5_000);
  await proof;
  expect(jest.getTimerCount()).toBe(0);
});

/** Return installed source unchanged and retire its request timeout after successful loading. */
test('returns successful native module source without retaining a request timer', async () => {
  jest.useFakeTimers();
  const source = 'export const native = true;';
  const request = jest
    .fn<ComfyRuntimeAssetRequester>()
    .mockResolvedValue({ ok: true, status: 200, text: async () => source });
  expect(await fetchComfyRuntimeText('/assets/GraphView.js', request)).toBe(source);
  expect(request).toHaveBeenCalledTimes(1);
  expect(jest.getTimerCount()).toBe(0);
});

/** Keep evaluated-code and module-resolution errors out of the automatic connection retry loop. */
test('distinguishes module download failure from invalid imports and programming exceptions', () => {
  expect(
    isComfyModuleTransportFailure(
      new TypeError('Failed to fetch dynamically imported module: /assets/native.js'),
    ),
  ).toBe(true);
  expect(
    isComfyModuleTransportFailure(
      new TypeError('error loading dynamically imported module: /assets/native.js'),
    ),
  ).toBe(true);
  expect(isComfyModuleTransportFailure(new TypeError('Importing a module script failed.'))).toBe(
    true,
  );
  expect(isComfyModuleTransportFailure(new TypeError('Failed to resolve module specifier'))).toBe(
    false,
  );
  expect(
    isComfyModuleTransportFailure(
      new TypeError("Cannot read properties of undefined (reading 'import')"),
    ),
  ).toBe(false);
  expect(isComfyModuleTransportFailure(new Error('Failed to fetch'))).toBe(false);
});
