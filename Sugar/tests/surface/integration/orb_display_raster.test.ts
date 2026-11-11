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
/** Prove cached physical-pixel orb variants, density observation and browser-resource cleanup. */
import { afterEach, expect, jest, test } from '@jest/globals';
import { AppOrbRenderer } from '../../../frontend/comfyui/ui/surface/frame/AppOrbRenderer.js';
import { createAccentPalette } from '../../../frontend/comfyui/ui/surface/appearance/AccentPalette.js';

const densityDescriptor = Object.getOwnPropertyDescriptor(window, 'devicePixelRatio');
const mediaDescriptor = Object.getOwnPropertyDescriptor(window, 'matchMedia');
const decodeDescriptor = Object.getOwnPropertyDescriptor(HTMLImageElement.prototype, 'decode');
afterEach(() => {
  jest.restoreAllMocks();
  for (const [owner, key, descriptor] of [
    [window, 'devicePixelRatio', densityDescriptor],
    [window, 'matchMedia', mediaDescriptor],
    [HTMLImageElement.prototype, 'decode', decodeDescriptor],
  ] as const) {
    if (descriptor) Object.defineProperty(owner, key, descriptor);
    else Reflect.deleteProperty(owner, key);
  }
});

/** Supply the missing JSDOM raster adapter while keeping all composition orchestration real. */
function mount(opaqueSource = false) {
  let density = 1;
  Object.defineProperty(window, 'devicePixelRatio', { configurable: true, get: () => density });
  const listeners = new Set<() => void>();
  Object.defineProperty(window, 'matchMedia', {
    configurable: true,
    value: () => ({
      addEventListener: (_type: string, listener: () => void) => listeners.add(listener),
      removeEventListener: (_type: string, listener: () => void) => listeners.delete(listener),
    }),
  });
  const decode = jest.fn<() => Promise<void>>().mockResolvedValue(undefined);
  Object.defineProperty(HTMLImageElement.prototype, 'decode', {
    configurable: true,
    value: decode,
  });
  const raster = (width: number, height: number): ImageData => ({
    data: new Uint8ClampedArray(width * height * 4).fill(opaqueSource ? 255 : 0),
    width,
    height,
    colorSpace: 'srgb',
  });
  const context = {
    drawImage: jest.fn(),
    clearRect: jest.fn(),
    getImageData: jest.fn((_x: number, _y: number, width: number, height: number) =>
      raster(width, height),
    ),
    createImageData: jest.fn((width: number, height: number) => raster(width, height)),
    putImageData: jest.fn<(image: ImageData, x: number, y: number) => void>(),
    imageSmoothingEnabled: false,
    imageSmoothingQuality: 'low',
  };
  // JSDOM has no canvas backend; this boundary supplies exactly the renderer's raster API surface.
  jest
    .spyOn(HTMLCanvasElement.prototype, 'getContext')
    .mockReturnValue(context as unknown as CanvasRenderingContext2D);
  const report = jest.fn();
  const renderer = new AppOrbRenderer(document, report);
  return {
    renderer,
    context,
    decode,
    report,
    listeners,
    setDensity: (next: number) => {
      density = next;
      [...listeners].forEach((listener) => listener());
    },
  };
}

test.each([true, false])('orb layers use the shared button shades for dark=%s', async (dark) => {
  const f = mount(true);
  await f.renderer.update('#0078d4', dark);
  const p = createAccentPalette('#0078d4', dark);
  for (const [index, shade] of [p.button.rest, p.button.hover, p.button.pressed].entries()) {
    const pixel = f.context.putImageData.mock.calls[index * 4]?.[0].data.slice(0, 4);
    expect(Array.from(pixel ?? [])).toEqual([
      Number.parseInt(shade.slice(1, 3), 16),
      Number.parseInt(shade.slice(3, 5), 16),
      Number.parseInt(shade.slice(5, 7), 16),
      255,
    ]);
  }
  f.renderer.dispose();
});

test('raster variants are downsampled once to display pixels and pointer changes only blit cached images', async () => {
  const f = mount();
  await f.renderer.update('#4080c0', true);
  expect(f.renderer.canvas.width).toBe(46);
  expect(f.renderer.canvas.height).toBe(46);
  expect(f.decode).toHaveBeenCalledTimes(4);
  expect(f.context.createImageData).toHaveBeenCalledTimes(3);
  expect(f.context.createImageData).toHaveBeenCalledWith(46, 46);
  const resamples = f.context.putImageData.mock.calls.length;
  f.renderer.setState('hover');
  f.renderer.setState('pressed');
  f.renderer.setState('idle');
  await f.renderer.update('#4080c0', true);
  expect(f.context.putImageData).toHaveBeenCalledTimes(resamples);
  f.setDensity(2);
  await f.renderer.update('#4080c0', true);
  expect(f.renderer.canvas.width).toBe(92);
  expect(f.decode).toHaveBeenCalledTimes(4);
  expect(f.context.createImageData).toHaveBeenCalledWith(92, 92);
  f.renderer.dispose();
  expect(f.listeners.size).toBe(0);
  const draws = f.context.drawImage.mock.calls.length;
  f.renderer.setState('hover');
  expect(f.context.drawImage).toHaveBeenCalledTimes(draws);
});

test('density refresh reports raster failures through the surface owner', async () => {
  const f = mount();
  await f.renderer.update('#4080c0', true);
  const failure = new Error('raster unavailable');
  f.context.getImageData.mockImplementation(() => {
    throw failure;
  });
  f.setDensity(2);
  await Promise.resolve();
  expect(f.report).toHaveBeenCalledWith(failure);
  f.renderer.dispose();
});
