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
/** Prove palette changes select neutral icon variants across all owned surfaces without replacing artwork. */
import { afterEach, expect, test } from '@jest/globals';
import { CubeIconAppearanceHost } from '../../../frontend/comfyui/ui/surface/appearance/CubeIconAppearanceHost.js';
import { FluentAppearanceState } from '../../../frontend/comfyui/ui/surface/appearance/FluentAppearanceState.js';

afterEach(() => {
  document.head.replaceChildren();
  document.body.replaceChildren();
});

test.each([
  'sugarcubes-cube-face__icon',
  'sugarcubes-workflow-masonry__cube-icon',
  'sc-cube-stack-row__icon',
  'sugarcubes-cube-icon',
])('adapts %s in either appearance mode while retaining image and canvas identity', (className) => {
  const source = new FluentAppearanceState();
  const host = new CubeIconAppearanceHost(document, source);
  const layout = document.createElement('style');
  layout.textContent = `.${className} img, .${className} canvas { display:block; }`;
  document.head.append(layout);
  const icon = document.createElement('span');
  icon.className = className;
  icon.dataset.iconTone = 'bright';
  const image = document.createElement('img');
  const variant = document.createElement('canvas');
  variant.className = 'sugarcubes-cube-icon__contrast';
  icon.append(image, variant);
  document.body.append(icon);
  for (const fluent of [false, true]) {
    source.update(fluent, false);
    expect(getComputedStyle(image).display).toBe('none');
    expect(getComputedStyle(variant).display).toBe('block');
    source.update(fluent, true);
    expect(getComputedStyle(image).display).not.toBe('none');
    expect(getComputedStyle(variant).display).toBe('none');
    icon.dataset.iconTone = 'dark';
    source.update(fluent, false);
    expect(getComputedStyle(image).display).not.toBe('none');
    source.update(fluent, true);
    expect(getComputedStyle(image).display).toBe('none');
    icon.dataset.iconTone = 'original';
    source.update(fluent, false);
    expect(getComputedStyle(image).display).not.toBe('none');
    expect(getComputedStyle(variant).display).toBe('none');
    expect(icon.firstChild).toBe(image);
    icon.dataset.iconTone = 'bright';
  }
  host.dispose();
  expect(document.getElementById('sugarcubes-cube-icon-appearance')).toBeNull();
  source.update(false, false);
  expect(document.getElementById('sugarcubes-cube-icon-appearance')).toBeNull();
});

test('keeps fallback initials readable on light surfaces and restores their original dark treatment', () => {
  const source = new FluentAppearanceState();
  const host = new CubeIconAppearanceHost(document, source);
  const canvas = document.createElement('canvas');
  canvas.className = 'sugarcubes-cube-icon__fallback';
  document.body.append(canvas);
  source.update(false, false);
  expect(getComputedStyle(canvas).filter).toBe('brightness(0)');
  source.update(false, true);
  expect(getComputedStyle(canvas).filter).toBe('none');
  host.dispose();
});
