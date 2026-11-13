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
/** Safeguard row labels, padding and grouped-row gaps without claiming unrelated controls. */
import { expect, test } from '@jest/globals';
import { dimensionFieldTarget } from '../../../frontend/comfyui/ui/editor_actions/presentation/DimensionFieldTarget.js';
import type { NativeEditorField } from '../../../frontend/comfyui/ui/editor_actions/infrastructure/NativeEditorCardAdapter.js';

/** Describe one bounded numeric column through the same field surface used by menus. */
function column(name: string, left: number, top = 20): NativeEditorField {
  const element = document.createElement('div');
  Object.assign(element, {
    getBoundingClientRect: () => ({
      left,
      right: left + 100,
      top,
      bottom: top + 40,
      width: 100,
      height: 40,
    }),
  });
  return {
    name,
    kind: 'number',
    element,
    read: () => 1024,
    write: () => true,
    disabled: () => false,
  };
}

test('routes labels and padding to their field and inter-column gaps to the nearest side', () => {
  const root = document.createElement('div'),
    group = document.createElement('div');
  const width = column('width', 10),
    height = column('height', 130);
  group.append(width.element, height.element);
  root.append(group);
  const label = document.createElement('label');
  height.element.append(label);
  const fields = [width, height];
  expect(dimensionFieldTarget(root, fields, label, 140, 25)).toBe(height);
  expect(dimensionFieldTarget(root, fields, width.element, 12, 25)).toBe(width);
  expect(dimensionFieldTarget(root, fields, group, 118, 25)).toBe(width);
  expect(dimensionFieldTarget(root, fields, group, 122, 25)).toBe(height);
  expect(dimensionFieldTarget(root, fields, group, 120, 25)).toBe(width);
  expect(dimensionFieldTarget(root, fields, group, 120, 90)).toBeNull();
  expect(dimensionFieldTarget(root, fields, root, 120, 25)).toBeNull();
  const unrelated = document.createElement('input');
  group.append(unrelated);
  expect(dimensionFieldTarget(root, fields, unrelated, 120, 25)).toBeNull();
});

test('does not invent a grouped row for vertically stacked dimensions or hidden fields', () => {
  const root = document.createElement('div'),
    group = document.createElement('div');
  const width = column('width', 10),
    height = column('height', 10, 80);
  group.append(width.element, height.element);
  root.append(group);
  expect(dimensionFieldTarget(root, [width, height], group, 50, 50)).toBeNull();
  expect(
    dimensionFieldTarget(root, [{ ...width, element: root }, height], root, 50, 30),
  ).toBeNull();
});
