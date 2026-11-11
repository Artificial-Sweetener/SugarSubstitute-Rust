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
/** Prove cancellable renderer recovery with backoff while retaining host renderer ownership. */
import { afterEach, beforeEach, expect, jest, test } from '@jest/globals';
import { NativeRendererAcquisition } from '../../../frontend/comfyui/ui/surface/NativeRendererAcquisition.js';
import { ComfyRuntimeAssetUnavailableError } from '../../../frontend/comfyui/ui/surface/ComfyRuntimeAssetRequest.js';
import type { NativeNodeCardRenderer } from '../../../frontend/comfyui/ui/surface/NativeNodeCardRenderer.js';

const renderer: NativeNodeCardRenderer = {
  mount: () => ({ refresh: () => undefined, unmount: () => undefined }),
  dispose: jest.fn(),
};
beforeEach(() => {
  jest.useFakeTimers();
});
afterEach(() => {
  jest.useRealTimers();
});

/** Resume one desired presentation after the server recovers, without repeated user-facing failures. */
test('backs off temporary failures and returns the same native renderer after recovery', async () => {
  const failure = new ComfyRuntimeAssetUnavailableError(
    '/assets/GraphView.js',
    new TypeError('Failed to fetch'),
  );
  const load = jest
    .fn<() => Promise<NativeNodeCardRenderer>>()
    .mockRejectedValueOnce(failure)
    .mockRejectedValueOnce(failure)
    .mockResolvedValue(renderer);
  const logger = { warn: jest.fn() };
  const recovery = new NativeRendererAcquisition(document, logger);
  const pending = recovery.acquire(load);
  await jest.advanceTimersByTimeAsync(499);
  expect(load).toHaveBeenCalledTimes(1);
  await jest.advanceTimersByTimeAsync(1);
  expect(load).toHaveBeenCalledTimes(2);
  await jest.advanceTimersByTimeAsync(1000);
  expect(await pending).toBe(renderer);
  expect(logger.warn).toHaveBeenCalledTimes(1);
  expect(jest.getTimerCount()).toBe(0);
});

/** Stop every scheduled attempt when the selected presentation or workflow departs. */
test('cancels an offline presentation without retrying or disposing a shared renderer', async () => {
  const load = jest
    .fn<() => Promise<NativeNodeCardRenderer>>()
    .mockRejectedValue(
      new ComfyRuntimeAssetUnavailableError(
        '/assets/GraphView.js',
        new TypeError('Failed to fetch'),
      ),
    );
  const recovery = new NativeRendererAcquisition(document, { warn: jest.fn() });
  const pending = recovery.acquire(load);
  await jest.advanceTimersByTimeAsync(0);
  recovery.cancel();
  expect(await pending).toBeNull();
  await jest.advanceTimersByTimeAsync(20_000);
  expect(load).toHaveBeenCalledTimes(1);
  expect(jest.getTimerCount()).toBe(0);
  expect(renderer.dispose).not.toHaveBeenCalled();
});

/** Ignore late assets from an obsolete request without invalidating their runtime owner. */
test('ignores a late resolution after a new presentation takes over', async () => {
  let resolve: (value: NativeNodeCardRenderer) => void = () => undefined;
  const delayed = new Promise<NativeNodeCardRenderer>((done) => {
    resolve = done;
  });
  const recovery = new NativeRendererAcquisition(document, { warn: jest.fn() });
  const obsolete = recovery.acquire(() => delayed);
  expect(await recovery.acquire(async () => renderer)).toBe(renderer);
  resolve(renderer);
  expect(await obsolete).toBeNull();
  expect(renderer.dispose).not.toHaveBeenCalled();
});

/** Preserve fail-closed handling for actual component incompatibility and programming failures. */
test('propagates non-transport failures immediately without allocating retry timers', async () => {
  const failure = new Error('Unsupported native renderer');
  const recovery = new NativeRendererAcquisition(document, { warn: jest.fn() });
  await expect(
    recovery.acquire(async () => {
      throw failure;
    }),
  ).rejects.toBe(failure);
  expect(jest.getTimerCount()).toBe(0);
});
