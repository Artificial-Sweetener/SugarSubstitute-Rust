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
/** Describe editable fields through explicit host-owned reads, constraints and writes. */
import type { PresetValue } from '../domain/UserPreset.js';
import { copySavablePresetValue } from '../domain/PresetCodec.js';
export interface EditorField {
  readonly name: string;
  readonly kind: 'number' | 'string' | 'boolean' | 'combo' | 'json';
  read(): unknown;
  disabled(): boolean;
  write(value: PresetValue): boolean;
}
/** Capture detached editable JSON inputs without persisting execution controls or connections. */
export function captureEditorInputs(fields: readonly EditorField[]): Record<string, PresetValue> {
  return Object.fromEntries(
    fields.flatMap((field) => {
      if (field.disabled()) return [];
      const value = copySavablePresetValue(field.read());
      if (value === undefined) return [];
      return [[field.name, value]];
    }),
  );
}
/** Apply matching inputs through host owners while reporting skipped fields for diagnostics. */
export function applyEditorInputs(
  fields: readonly EditorField[],
  inputs: Readonly<Record<string, PresetValue>>,
): string[] {
  const skipped: string[] = [];
  for (const [name, value] of Object.entries(inputs)) {
    const field = fields.find((field) => field.name === name);
    const detached = copySavablePresetValue(value);
    if (!field || detached === undefined || !field.write(detached)) skipped.push(name);
  }
  return skipped;
}
