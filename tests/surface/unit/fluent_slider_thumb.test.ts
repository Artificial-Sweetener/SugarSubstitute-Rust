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
/** Prove the decorative slider thumb paints its ring and animated dot on one centered surface. */
import { afterEach, expect, test } from '@jest/globals';
import {
  createFluentSliderThumb,
  fluentSliderThumbCss,
} from '../../../frontend/comfyui/ui/surface/appearance/FluentSliderThumb.js';

afterEach(() => document.body.replaceChildren());

test('uses Fluent’s thumb slot without introducing another input or focus target', () => {
  const thumb = createFluentSliderThumb(document);
  expect(thumb.slot).toBe('thumb');
  expect(thumb.getAttribute('aria-hidden')).toBe('true');
  expect(thumb.childElementCount).toBe(0);
  expect(thumb.querySelector('input, button, [tabindex], [role]')).toBeNull();
  expect(thumb.hasAttribute('tabindex')).toBe(false);
});

test('centers dot painting in the fixed ring for hover and press, reduced motion and high contrast', () => {
  const style = document.createElement('style');
  style.textContent = fluentSliderThumbCss();
  const thumb = createFluentSliderThumb(document);
  document.body.append(style, thumb);
  const appearance = getComputedStyle(thumb);
  expect(appearance.display).toBe('block');
  expect(appearance.boxSizing).toBe('border-box');
  expect(appearance.backgroundPosition).toBe('center');
  expect(appearance.backgroundSize).toBe('10px 10px');
  expect(appearance.backgroundRepeat).toBe('no-repeat');
  expect(appearance.transform).toBe('');
  const rules = Array.from(style.sheet?.cssRules ?? [])
    .map((rule) => rule.cssText)
    .join('\n');
  expect(rules).toContain(':not([disabled]) [data-sc-slider-thumb]:hover');
  expect(rules).toContain('background-size: 12px 12px');
  expect(rules).toContain(':not([disabled]):active');
  expect(rules).toContain('background-size: 8px 8px');
  expect(rules).toContain('radial-gradient(circle closest-side');
  expect(rules).toContain('prefers-reduced-motion:reduce');
  expect(rules).toContain('forced-colors:active');
});
