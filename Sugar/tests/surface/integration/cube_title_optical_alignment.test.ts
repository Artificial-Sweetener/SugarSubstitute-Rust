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
/** Prove mounted title alignment updates when fonts load without retaining retired titles. */
import { expect, jest, test } from '@jest/globals';
import {
  alignCubeTitleOptically,
  refreshCubeTitleOpticalAlignment,
} from '../../../frontend/comfyui/ui/surface/CubeTitleOpticalDomAlignment.js';

test('measures capitals for both font sizes, keeps geometry intact, and refreshes loaded fonts', () => {
  const frames: FrameRequestCallback[] = [];
  const schedule = jest.spyOn(window, 'requestAnimationFrame').mockImplementation((callback) => {
    frames.push(callback);
    return frames.length;
  });
  const originalConstructor = Object.getOwnPropertyDescriptor(
    globalThis,
    'CanvasRenderingContext2D',
  );
  const originalFonts = Object.getOwnPropertyDescriptor(document, 'fonts');
  Object.defineProperty(globalThis, 'CanvasRenderingContext2D', {
    configurable: true,
    value: class {},
  });
  const fonts = new EventTarget();
  Object.defineProperty(document, 'fonts', { configurable: true, value: fonts });
  let capRatio = 0.65;
  const context = {
    font: '',
    textBaseline: 'alphabetic',
    measureText: jest.fn((text: string) => {
      expect(text).toBe('H');
      const size = Number.parseFloat(context.font.match(/([\d.]+)px/)?.[1] ?? '12');
      return {
        actualBoundingBoxAscent: size * capRatio,
        actualBoundingBoxDescent: 0,
        fontBoundingBoxAscent: size,
        fontBoundingBoxDescent: size * 0.25,
      };
    }),
  };
  // Isolate the external Canvas API while exercising real font events, DOM styles, and scheduling.
  const canvas = jest
    .spyOn(HTMLCanvasElement.prototype, 'getContext')
    .mockReturnValue(context as unknown as CanvasRenderingContext2D);
  try {
    const title = document.createElement('span');
    const label = document.createElement('span');
    label.className = 'sugarcubes-model-title__pill-label';
    label.style.font = '700 12px Arial';
    label.textContent = 'Anima';
    const name = document.createElement('span');
    name.className = 'sugarcubes-model-title__name';
    name.style.font = '400 16px Arial';
    name.textContent = 'Typography with descenders gyp';
    title.append(label, name);
    document.body.append(title);
    alignCubeTitleOptically(title);
    expect(frames).toHaveLength(1);
    frames.shift()?.(0);
    expect(Number.parseFloat(label.style.getPropertyValue('--sc-cap-line-offset'))).toBeCloseTo(
      -0.6,
    );
    expect(Number.parseFloat(name.style.getPropertyValue('--sc-cap-line-offset'))).toBeCloseTo(
      -0.8,
    );
    expect(label.style.height).toBe('');
    expect(name.textContent).toBe('Typography with descenders gyp');
    name.style.font = '400 20px Arial';
    refreshCubeTitleOpticalAlignment(document);
    refreshCubeTitleOpticalAlignment(document);
    expect(frames).toHaveLength(1);
    frames.shift()?.(0);
    expect(Number.parseFloat(name.style.getPropertyValue('--sc-cap-line-offset'))).toBeCloseTo(-1);
    capRatio = 0.75;
    fonts.dispatchEvent(new Event('loadingdone'));
    frames.shift()?.(0);
    expect(Number.parseFloat(label.style.getPropertyValue('--sc-cap-line-offset'))).toBeCloseTo(0);
    expect(Number.parseFloat(name.style.getPropertyValue('--sc-cap-line-offset'))).toBeCloseTo(0);
    const calls = context.measureText.mock.calls.length;
    title.remove();
    fonts.dispatchEvent(new Event('loadingdone'));
    frames.shift()?.(0);
    expect(context.measureText).toHaveBeenCalledTimes(calls);
  } finally {
    schedule.mockRestore();
    canvas.mockRestore();
    if (originalConstructor)
      Object.defineProperty(globalThis, 'CanvasRenderingContext2D', originalConstructor);
    else Reflect.deleteProperty(globalThis, 'CanvasRenderingContext2D');
    if (originalFonts) Object.defineProperty(document, 'fonts', originalFonts);
    else Reflect.deleteProperty(document, 'fonts');
    document.body.replaceChildren();
  }
});
