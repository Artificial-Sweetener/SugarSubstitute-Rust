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
/** Prove the animated slider thumb stays decorative and keeps fixed geometry around its inner dot. */
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
  expect(thumb.querySelectorAll('[data-sc-slider-thumb-center]')).toHaveLength(1);
  expect(thumb.querySelector('input, button, [tabindex], [role]')).toBeNull();
  expect(thumb.hasAttribute('tabindex')).toBe(false);
});

test('sizes only the inner dot for enabled hover and press, with reduced motion and high contrast support', () => {
  const style = document.createElement('style');
  style.textContent = fluentSliderThumbCss();
  const thumb = createFluentSliderThumb(document);
  document.body.append(style, thumb);
  const center = thumb.querySelector('[data-sc-slider-thumb-center]');
  if (!center) throw new Error('Missing thumb center.');
  expect(getComputedStyle(thumb).display).toBe('grid');
  expect(getComputedStyle(center).width).toBe('10px');
  expect(getComputedStyle(center).height).toBe('10px');
  expect(getComputedStyle(center).pointerEvents).toBe('none');
  expect(getComputedStyle(center).transform).toBe('scale(1)');
  const rules = Array.from(style.sheet?.cssRules ?? [])
    .map((rule) => rule.cssText)
    .join('\n');
  expect(rules).toContain(':not([disabled]) [data-sc-slider-thumb]:hover');
  expect(rules).toContain('transform: scale(1.2)');
  expect(rules).toContain(':not([disabled]):active');
  expect(rules).toContain('transform: scale(.8)');
  expect(rules).toContain('prefers-reduced-motion:reduce');
  expect(rules).toContain('forced-colors:active');
});
