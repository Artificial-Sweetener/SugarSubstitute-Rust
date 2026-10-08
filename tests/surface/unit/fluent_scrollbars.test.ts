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
/** Safeguard the Fluent-wide overlay contract and absence of reserved native gutters. */
import { expect, test } from '@jest/globals';
import { fluentAppearanceCss } from '../../../frontend/comfyui/ui/surface/appearance/FluentAppearanceStyles.js';
import {
  fluentScrollbarCss,
  fluentScrollbarPresentationCss,
} from '../../../frontend/comfyui/ui/surface/appearance/FluentScrollbarStyles.js';
import {
  scrollbarGeometry,
  scrollbarDragPosition,
} from '../../../frontend/comfyui/ui/surface/appearance/FluentScrollbarGeometry.js';

test.each([true, false])(
  'enforces floating scrollbars across Fluent mode in palette %s',
  (dark) => {
    const css = fluentAppearanceCss(dark);
    expect(css).toContain('scrollbar-width:none!important');
    expect(css).toContain('scrollbar-gutter:auto!important');
    expect(css).not.toContain('scrollbar-width:auto!important');
    expect(css).not.toContain('scrollbar-width:thin!important');
    expect(css).not.toContain('data-sc-horizontal-scrollbar');
    expect(fluentScrollbarCss(dark)).not.toContain('padding-bottom');
    expect(fluentScrollbarCss(dark)).not.toContain('padding-block-end');
    expect(css).toContain('::-webkit-scrollbar');
    expect(css).toContain('.sc-fluent-scrollbar-layer');
    expect(css).toContain('position:absolute');
    expect(css).toContain('z-index:1');
    expect(css).toContain('overflow:clip');
    expect(css).not.toContain('2147483646');
  },
);

test('bounds the thumb and maps native scroll range to pointer travel', () => {
  const geometry = scrollbarGeometry(200, 200, 1000, 400);
  expect(geometry).toEqual({ length: 40, offset: 80, travel: 160, maximum: 800 });
  if (!geometry) throw new Error('Expected overflow geometry');
  expect(scrollbarDragPosition(geometry, 400, 80)).toBe(800);
  expect(scrollbarDragPosition(geometry, 400, -200)).toBe(0);
  expect(scrollbarGeometry(20, 20, 10000, 9000)).toEqual({
    length: 20,
    offset: 0,
    travel: 0,
    maximum: 9980,
  });
  expect(scrollbarGeometry(100, 100, 100, 0)).toBeNull();
  expect(scrollbarGeometry(0, 100, 200, 0)).toBeNull();
  expect(scrollbarGeometry(100, 100, Infinity, 0)).toBeNull();
});

test('keeps horizontal paint thin and flush with the bottom while preserving vertical paint', () => {
  const css = fluentScrollbarPresentationCss();
  expect(css).toContain('height:calc(2px / var(--sc-scrollbar-scale-y,1))');
  expect(css).toContain('bottom:0;');
  expect(css).toContain('height:calc(4px / var(--sc-scrollbar-scale-y,1))');
  expect(css).toContain('width:calc(3px / var(--sc-scrollbar-scale-x,1))');
});
