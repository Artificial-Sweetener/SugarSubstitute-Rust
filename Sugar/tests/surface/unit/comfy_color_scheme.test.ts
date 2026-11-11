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
/** Verify owned surfaces follow palette colors even when host theme metadata is stale. */
import { afterEach, expect, test } from '@jest/globals';
import { readOwnedDarkTheme } from '../../../frontend/comfyui/ui/surface/appearance/OwnedColorScheme.js';

afterEach(() => {
  document.documentElement.removeAttribute('style');
  document.documentElement.className = '';
  document.documentElement.removeAttribute('data-sc-visual-style');
  document.documentElement.removeAttribute('data-sc-photopro-palette');
});

test.each(['#F5F5F5', '#fff', 'rgb(245 245 245)'])(
  'recognizes a light custom surface %s under a dark host flag',
  (background) => {
    document.documentElement.className = 'dark-theme';
    document.documentElement.style.colorScheme = 'dark';
    document.documentElement.style.setProperty('--comfy-menu-bg', background);
    expect(readOwnedDarkTheme(document)).toBe(false);
  },
);

test.each(['#202020', '#002b36', 'rgb(46, 52, 64)'])(
  'recognizes a dark custom surface %s under a light host flag',
  (background) => {
    document.documentElement.style.colorScheme = 'light';
    document.documentElement.style.setProperty('--comfy-menu-bg', background);
    expect(readOwnedDarkTheme(document)).toBe(true);
  },
);

test('falls back to the host color scheme when no usable opaque palette surface is available', () => {
  for (const background of ['', 'transparent', 'rgba(0, 0, 0, 0)', 'var(--unknown)']) {
    document.documentElement.style.setProperty('--comfy-menu-bg', background);
    document.documentElement.style.colorScheme = 'light';
    expect(readOwnedDarkTheme(document)).toBe(false);
    document.documentElement.style.colorScheme = 'dark';
    expect(readOwnedDarkTheme(document)).toBe(true);
  }
});

test.each(['black', 'dark-gray', 'medium-gray', 'light-gray'])(
  'PhotoPro %s ignores an opposite Comfy color scheme',
  (palette) => {
    const dark = palette !== 'light-gray';
    document.documentElement.dataset.scVisualStyle = 'photopro';
    document.documentElement.dataset.scPhotoproPalette = palette;
    document.documentElement.style.colorScheme = dark ? 'light' : 'dark';
    document.documentElement.style.setProperty('--comfy-menu-bg', dark ? '#ffffff' : '#202020');
    expect(readOwnedDarkTheme(document)).toBe(dark);
    document.documentElement.dataset.scVisualStyle = 'fluent';
    expect(readOwnedDarkTheme(document)).toBe(!dark);
  },
);
