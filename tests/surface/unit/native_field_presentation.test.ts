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
    expect(classifyNativeField({ name: 'ckpt_name', component: 'WidgetSelect' })).toBe('model');
    expect(classifyNativeField({ name: 'text', component: 'WidgetTextarea' })).toBe('multiline');
  });
});

test('field-row structure is scoped to masonry while shared theme tokens remain available to graph cards', () => {
  const css = fluentAppearanceCss(true);
  expect(css).toContain('.sugarcubes-workflow-masonry [data-sc-fluent-fields]');
  expect(css).toContain('.sugarcubes-workflow-masonry [data-sc-fluent-row]');
  expect(css).not.toMatch(/\n\s*\[data-sc-fluent-fields\]/);
  expect(css).toContain('--component-node-background:var(--sc-card-fill)');
});
