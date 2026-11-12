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
/** Prove physical-size quality selection, settled geometry and bounded native decoding reuse. */
import { jest } from '@jest/globals';
import { inputPreviewResolution } from '../../../frontend/comfyui/ui/input_canvas/domain/InputPreviewResolution.js';
import { InputPreviewResolutionObserver } from '../../../frontend/comfyui/ui/input_canvas/presentation/InputPreviewResolutionObserver.js';
import { InputPreviewWorkCache } from '../../../frontend/comfyui/ui/input_canvas/application/InputPreviewWorkCache.js';
import { sourceCoverage } from '../support/source_mask_fixture.js';
import { MemoryBudgetRegistry } from '../../../frontend/comfyui/ui/core/memory/MemoryBudgetRegistry.js';

/** Settle queued decoder admission and completion without real time. */
async function settled(): Promise<void> {
  for (let n = 0; n < 20; n++) await Promise.resolve();
}

test('buckets CSS size times pixel density and caps the raster at one megapixel', () => {
  expect(inputPreviewResolution(288, 512, 1)).toBe(512);
  expect(inputPreviewResolution(288, 512, 2)).toBe(1024);
  expect(inputPreviewResolution(44, 44, 1)).toBe(64);
  expect(inputPreviewResolution(9000, 9000, 4)).toBe(1024);
  expect(inputPreviewResolution(0, 0, 1)).toBe(0);
  expect(inputPreviewResolution(NaN, 10, 1)).toBe(0);
});

test('coalesces geometry changes, uses device pixels and retires a pending upgrade on disposal', () => {
  jest.useFakeTimers();
  const descriptor = Object.getOwnPropertyDescriptor(window, 'ResizeObserver');
  const densityDescriptor = Object.getOwnPropertyDescriptor(window, 'devicePixelRatio');
  let callback: ResizeObserverCallback = () => {};
  let native: ResizeObserver | null = null;
  class Observer implements ResizeObserver {
    /** Capture the external observer callback for deterministic physical-size notifications. */
    constructor(changed: ResizeObserverCallback) {
      callback = changed;
      native = this;
    }
    /** Accept the actual observed content box. */
    observe(): void {}
    /** Release the external observer. */
    disconnect(): void {}
    /** Satisfy the native observer boundary. */
    unobserve(): void {}
  }
  Object.defineProperty(window, 'ResizeObserver', { configurable: true, value: Observer });
  const frame = document.createElement('div'),
    changed = jest.fn(),
    observer = new InputPreviewResolutionObserver(frame, changed);
  /** Notify the native boundary with CSS and physical pixel dimensions. */
  const size = (width: number, height: number, physical: number) => {
    if (!native) throw new Error('Missing native observer.');
    callback(
      [
        {
          target: frame,
          contentRect: {
            x: 0,
            y: 0,
            top: 0,
            left: 0,
            right: width,
            bottom: height,
            width,
            height,
            toJSON: () => ({ width, height }),
          },
          borderBoxSize: [],
          contentBoxSize: [],
          devicePixelContentBoxSize: [
            { inlineSize: width * physical, blockSize: height * physical },
          ],
        },
      ],
      native,
    );
  };
  try {
    expect(observer.resolution).toBe(0);
    Object.defineProperty(window, 'devicePixelRatio', { configurable: true, value: 2 });
    size(288, 512, 1);
    size(288, 512, 1);
    jest.advanceTimersByTime(119);
    expect(changed).not.toHaveBeenCalled();
    jest.advanceTimersByTime(1);
    expect(observer.resolution).toBe(1024);
    expect(changed).toHaveBeenCalledTimes(1);
    size(44, 44, 1);
    jest.advanceTimersByTime(500);
    expect(changed).toHaveBeenCalledTimes(1);
    observer.dispose();
    const second = new InputPreviewResolutionObserver(frame, changed);
    size(900, 900, 1);
    second.dispose();
    jest.advanceTimersByTime(500);
    expect(changed).toHaveBeenCalledTimes(1);
  } finally {
    observer.dispose();
    if (descriptor) Object.defineProperty(window, 'ResizeObserver', descriptor);
    else Reflect.deleteProperty(window, 'ResizeObserver');
    if (densityDescriptor) Object.defineProperty(window, 'devicePixelRatio', densityDescriptor);
    else Reflect.deleteProperty(window, 'devicePixelRatio');
    jest.useRealTimers();
  }
});

test('shares in-flight decodes, limits concurrency and evicts old rasters without caching failures', async () => {
  const cache = new InputPreviewWorkCache(new MemoryBudgetRegistry()),
    releases: (() => void)[] = [];
  const load = jest.fn(async () => {
    await new Promise<void>((resolve) => releases.push(resolve));
    return sourceCoverage;
  });
  const first = cache.read('a', load);
  expect(cache.read('a', load)).toBe(first);
  const second = cache.read('b', load),
    third = cache.read('c', load);
  await settled();
  expect(load).toHaveBeenCalledTimes(2);
  releases.shift()?.();
  await settled();
  expect(load).toHaveBeenCalledTimes(3);
  releases.shift()?.();
  releases.shift()?.();
  await Promise.all([first, second, third]);
  expect(await cache.read('a', load)).toBe(sourceCoverage);
  expect(load).toHaveBeenCalledTimes(3);
  const immediate = jest.fn(async () => sourceCoverage);
  for (let n = 0; n < 17; n++) await cache.read(String(n), immediate);
  await cache.read('a', immediate);
  expect(immediate).toHaveBeenCalledTimes(18);
  const failure = new Error('Decode failed');
  await expect(
    cache.read('bad', async () => {
      throw failure;
    }),
  ).rejects.toBe(failure);
  await settled();
  await expect(cache.read('bad', immediate)).resolves.toBe(sourceCoverage);
});

test('rejects oversized decoded rasters and bounds pending work without caching invalid results', async () => {
  const cache = new InputPreviewWorkCache(new MemoryBudgetRegistry());
  await expect(
    cache.read('large', async () => ({ ...sourceCoverage, width: 1025 })),
  ).rejects.toThrow('size limit');
  await settled();
  await expect(cache.read('large', async () => sourceCoverage)).resolves.toBe(sourceCoverage);
  const accepted = Array.from({ length: 128 }, (_, n) =>
    cache.read('queued-' + String(n), async () => sourceCoverage),
  );
  await expect(cache.read('excess', async () => sourceCoverage)).rejects.toThrow(
    'Too many pending',
  );
  await Promise.all(accepted);
});

test('shares one budgeted cache between previews and exclusive full-resolution decodes', async () => {
  const memory = new MemoryBudgetRegistry();
  const cache = new InputPreviewWorkCache(memory),
    releases: (() => void)[] = [];
  const load = jest.fn(async () => {
    await new Promise<void>((resolve) => releases.push(resolve));
    return sourceCoverage;
  });
  const full = cache.read('full', load, true);
  const preview = cache.read('preview', load);
  await settled();
  expect(load).toHaveBeenCalledTimes(1);
  releases.shift()?.();
  await full;
  await settled();
  expect(load).toHaveBeenCalledTimes(2);
  releases.shift()?.();
  await preview;
  expect(memory.usage().consumers.map((consumer) => consumer.id)).toEqual([
    'input-canvas-previews',
  ]);
  expect(memory.usage().usedBytes).toBeGreaterThan(0);
  memory.setAllowance(memory.usage().usedBytes, 'critical');
  expect(memory.usage().usedBytes).toBe(0);
});
