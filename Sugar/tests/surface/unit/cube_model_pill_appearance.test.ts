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
/** Prove accent palette precedence and live theme changes without tinting title text. */
import { afterEach, expect, test } from '@jest/globals';
import { resolveCubeModelPillColors } from '../../../frontend/comfyui/ui/surface/CubeModelPillAppearance.js';

afterEach(() => {
  document.body.replaceChildren();
});

test('uses active Fluent colors, follows changes and returns to the native accent', () => {
  const canvas = document.createElement('canvas');
  document.body.append(canvas);
  canvas.style.setProperty('--p-primary-color', '#0078d4');
  canvas.style.setProperty('--p-primary-contrast-color', '#ffffff');
  canvas.style.setProperty('--comfy-menu-bg', '#202020');
  canvas.style.color = '#eeeeee';
  expect(resolveCubeModelPillColors(canvas)).toEqual({
    background: '#0078d4',
    foreground: '#202020',
  });
  canvas.style.setProperty('--sc-brand-text', '#479ef5');
  canvas.style.setProperty('--sc-text-on-accent', '#ffffff');
  expect(resolveCubeModelPillColors(canvas)).toEqual({
    background: '#479ef5',
    foreground: '#202020',
  });
  canvas.style.setProperty('--sc-brand-text', '#a4262c');
  expect(resolveCubeModelPillColors(canvas).background).toBe('#a4262c');
  canvas.style.removeProperty('--sc-brand-text');
  canvas.style.removeProperty('--sc-text-on-accent');
  expect(resolveCubeModelPillColors(canvas)).toEqual({
    background: '#0078d4',
    foreground: '#202020',
  });
  expect(canvas.style.color).toBe('rgb(238, 238, 238)');
});

test('uses the host accent or explicit defaults when theme tokens are absent', () => {
  const canvas = document.createElement('canvas');
  document.body.append(canvas);
  canvas.style.setProperty('--primary-color', '#663399');
  expect(resolveCubeModelPillColors(canvas)).toEqual({
    background: '#663399',
    foreground: '#171b20',
  });
  expect(resolveCubeModelPillColors()).toEqual({ background: '#60cdff', foreground: '#171b20' });
});

test('uses the shared accent and its readable foreground ahead of local Fluent and Comfy tokens', () => {
  const canvas = document.createElement('canvas');
  document.body.append(canvas);
  canvas.style.setProperty('--sc-accent-color', '#071530');
  canvas.style.setProperty('--sc-accent-foreground', '#ffffff');
  canvas.style.setProperty('--sc-brand-text', '#479ef5');
  canvas.style.setProperty('--comfy-menu-bg', '#202020');
  expect(resolveCubeModelPillColors(canvas)).toEqual({
    background: '#071530',
    foreground: '#ffffff',
  });
});
