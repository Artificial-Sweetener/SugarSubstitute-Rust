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
/** Safeguard detached custom JSON values and graph-connection exclusion at preset capture. */
import { expect, test } from '@jest/globals';
import {
  captureEditorInputs,
  applyEditorInputs,
} from '../../../frontend/comfyui/ui/editor_actions/application/EditorField.js';
import type { EditorField } from '../../../frontend/comfyui/ui/editor_actions/application/EditorField.js';

test('captures detached JSON custom values, including null, without retaining mutable widget state', () => {
  const value = { layers: [true, { weight: 0.5 }], mask: null };
  let applied: unknown;
  const field: EditorField = {
    name: 'layers',
    kind: 'json',
    disabled: () => false,
    read: () => value,
    write: (next) => {
      applied = next;
      return true;
    },
  };
  const inputs = captureEditorInputs([field]);
  expect(inputs).toEqual({ layers: value });
  value.layers.push(false);
  expect(inputs.layers).toEqual({ layers: [true, { weight: 0.5 }], mask: null });
  expect(applyEditorInputs([field], inputs)).toEqual([]);
  expect(applied).toEqual(inputs.layers);
  expect(applied).not.toBe(inputs.layers);
});

test('omits unsupported, connected, readonly, and non-finite inputs while retaining independent fields', () => {
  const values: Record<string, unknown> = {
    connection: ['12', 0],
    nestedConnection: { x: ['12', 0] },
    invalid: Infinity,
    callback: () => undefined,
    readonly: 'locked',
    empty: null,
  };
  const fields: EditorField[] = Object.entries(values).map(([name, value]) => ({
    name,
    kind: 'json',
    read: () => value,
    disabled: () => name === 'readonly',
    write: () => true,
  }));
  expect(captureEditorInputs(fields)).toEqual({ empty: null });
});
