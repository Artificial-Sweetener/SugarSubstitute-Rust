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
/** Prove Fluent's actual value calculation follows screen geometry across graph zoom levels. */
import { afterAll, beforeAll, expect, jest, test } from '@jest/globals';
import { createFluentNumericSlider } from '../../../frontend/comfyui/ui/surface/appearance/FluentThemeRuntime.js';
import { Slider } from '@fluentui/web-components/slider/class.js';
import { fluentNumericCss } from '../../../frontend/comfyui/ui/surface/appearance/FluentNumericStyles.js';

const internalsDescriptor = Object.getOwnPropertyDescriptor(
  HTMLElement.prototype,
  'attachInternals',
);

beforeAll(() => {
  // JSDOM lacks form-associated custom element internals; geometry uses only native layout.
  Object.defineProperty(HTMLElement.prototype, 'attachInternals', {
    configurable: true,
    value: () => ({ role: null, ariaOrientation: null }),
  });
});

afterAll(() => {
  if (internalsDescriptor)
    Object.defineProperty(HTMLElement.prototype, 'attachInternals', internalsDescriptor);
  else Reflect.deleteProperty(HTMLElement.prototype, 'attachInternals');
  jest.restoreAllMocks();
});

/** Supply the browser's measured track rectangle without relying on JSDOM layout. */
function trackRect(width: number): DOMRect {
  return {
    x: 0,
    y: 0,
    left: 0,
    top: 0,
    right: width,
    bottom: 4,
    width,
    height: 4,
    toJSON: () => ({ width }),
  };
}

test.each([0.5, 1, 2, 2.42, 3])('keeps slider values proportional at zoom %s', (zoom) => {
  const slider = createFluentNumericSlider(document);
  if (!(slider instanceof Slider)) throw new Error('Official Fluent slider missing');
  expect(slider.querySelector('[slot="thumb"][data-sc-slider-thumb]')).not.toBeNull();
  slider.min = '0';
  slider.max = '100';
  slider.step = '1';
  const track = document.createElement('div');
  Object.defineProperty(track, 'clientWidth', { value: 100 });
  jest.spyOn(track, 'getBoundingClientRect').mockReturnValue(trackRect(100 * zoom));
  slider.track = track;
  expect(slider.calculateNewValue(0)).toBe(0);
  expect(slider.calculateNewValue(25 * zoom)).toBe(25);
  expect(slider.calculateNewValue(50 * zoom)).toBe(50);
  expect(slider.calculateNewValue(75 * zoom)).toBe(75);
  expect(slider.calculateNewValue(100 * zoom)).toBe(100);
  jest.spyOn(track, 'getBoundingClientRect').mockReturnValue(trackRect(100));
  expect(slider.calculateNewValue(50)).toBe(50);
});

test('paints only the rounded native track between the endpoint thumb centers', () => {
  const css = fluentNumericCss();
  expect(css).toMatch(/\[data-sc-number-slider\]::after\s*\{\s*content:none;/);
  expect(css).toMatch(/::part\(track-container\)\s*\{\s*border-radius:var\(--border-radius\);/);
});

test('uses Fluent calculation safely before a measurable track is laid out', () => {
  const slider = createFluentNumericSlider(document);
  if (!(slider instanceof Slider)) throw new Error('Official Fluent slider missing');
  slider.min = '0';
  slider.max = '100';
  slider.step = '1';
  slider.track = document.createElement('div');
  expect(slider.calculateNewValue(0.5)).toBe(50);
});

test('preserves official Fluent control state across connected presentation moves', () => {
  for (const tag of ['fluent-slider', 'fluent-button', 'fluent-checkbox']) {
    const constructor = customElements.get(tag);
    expect(constructor).toBeDefined();
    if (!constructor) throw new Error('Fluent component registration missing');
    expect(typeof Reflect.get(constructor.prototype, 'connectedMoveCallback')).toBe('function');
  }
  const slider = createFluentNumericSlider(document);
  slider.value = '27';
  const callback: unknown = Reflect.get(slider, 'connectedMoveCallback');
  if (typeof callback !== 'function') throw new Error('Connected move callback missing');
  callback.call(slider);
  expect(slider.value).toBe('27');
});
