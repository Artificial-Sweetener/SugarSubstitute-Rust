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
/** Safeguard authored order, common grouping, and custom field boundaries. */
import { fluentAppearanceCss } from '../../../frontend/comfyui/ui/surface/appearance/FluentAppearanceStyles.js';
import { describe, expect, test } from '@jest/globals';
import {
  classifyNativeField,
  planNativeFields,
} from '../../../frontend/comfyui/ui/surface/appearance/NativeFieldPresentation.js';

test('sizes recognized model pickers through their owned mount and restores ordinary field geometry', () => {
  const style = document.createElement('style');
  style.textContent = fluentAppearanceCss(true);
  document.head.append(style);
  const surface = document.createElement('div');
  surface.className = 'sugarcubes-cube-face__node-card';
  const row = document.createElement('div');
  row.dataset.scFluentRow = '';
  const layout = document.createElement('div');
  layout.dataset.scFluentLayout = '';
  const label = document.createElement('label');
  label.textContent = 'diffusion_model';
  const mount = document.createElement('div');
  mount.className = 'sc-model-mount';
  const control = document.createElement('div');
  control.append(mount);
  layout.append(label, control);
  row.append(layout);
  surface.append(row);
  document.body.append(surface);
  try {
    expect(getComputedStyle(control).flexBasis).toBe('360px');
    mount.className = '';
    expect(getComputedStyle(control).flexBasis).toBe('156px');
    mount.className = 'sc-model-mount';
    surface.className = '';
    expect(getComputedStyle(control).flexBasis).toBe('');
  } finally {
    surface.remove();
    style.remove();
  }
});

describe('native Fluent field presentation', () => {
  test('groups adjacent dimensions and sampling controls without reordering source fields', () => {
    const fields = ['width', 'height', 'seed', 'steps', 'cfg', 'sampler_name', 'scheduler'].map(
      (name) => ({ name, component: 'WidgetInputNumber' }),
    );
    const result = planNativeFields(fields);
    expect(result.map((field) => field.name)).toEqual(fields.map((field) => field.name));
    expect(result.map((field) => [field.row, field.column, field.span])).toEqual([
      [1, 1, 1],
      [1, 2, 1],
      [2, 1, 2],
      [3, 1, 1],
      [3, 2, 1],
      [4, 1, 1],
      [4, 2, 1],
    ]);
  });
  test('preserves unknown/custom controls and prevents grouping across their boundaries', () => {
    const result = planNativeFields([
      { name: 'width', component: 'WidgetInputNumber' },
      { name: 'custom', component: 'WidgetDOM' },
      { name: 'height', component: 'WidgetInputNumber' },
    ]);
    expect(result.map((field) => [field.row, field.span])).toEqual([
      [1, 2],
      [3, 2],
    ]);
    expect(classifyNativeField({ name: 'seed', component: 'FancySeedEditor' })).toBeNull();
    expect(classifyNativeField({ name: 'ckpt_name', component: 'WidgetSelect' })).toBe('scalar');
    expect(classifyNativeField({ name: 'diffusion_model', component: 'WidgetSelect' })).toBe(
      'scalar',
    );
    expect(classifyNativeField({ name: 'text', component: 'WidgetTextarea' })).toBe('multiline');
  });
});

test('field-row structure is shared by projected cards while native graph cards retain host layout', () => {
  const css = fluentAppearanceCss(true);
  expect(css).toContain('.sugarcubes-cube-face__node-card [data-sc-fluent-fields]');
  expect(css).toContain('.sugarcubes-cube-face__node-card [data-sc-fluent-row]');
  expect(css).not.toMatch(/\n\s*\[data-sc-fluent-fields\]/);
  expect(css).toContain('--component-node-background:var(--sc-card-fill)');
});
