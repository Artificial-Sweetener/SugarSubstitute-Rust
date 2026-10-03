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
/** Resolve standard row geometry without claiming custom component presentation. */
export type NativeFieldKind = 'scalar' | 'multiline' | 'model';

const standardComponents: Readonly<Record<string, NativeFieldKind>> = {
  WidgetInputNumber: 'scalar',
  WidgetInputNumberInput: 'scalar',
  WidgetInputNumberSlider: 'scalar',
  WidgetToggleSwitch: 'scalar',
  WidgetSelect: 'scalar',
  WidgetInputText: 'scalar',
  WidgetTextarea: 'multiline',
};
const modelFields = new Set([
  'ckpt_name',
  'vae_name',
  'lora_name',
  'unet_name',
  'clip_name',
  'model_name',
]);
/** Enumerate the core components whose field layout this adapter understands. */
export const NATIVE_FIELD_COMPONENTS = Object.keys(standardComponents);

export interface NativeFieldDescriptor {
  name: string;
  component: string;
}

export interface NativeFieldPlacement {
  name: string;
  kind: NativeFieldKind;
  row: number;
  column: number;
  span: number;
}

/** Accept known native components rather than guessing from arbitrary input markup. */
export function classifyNativeField(field: NativeFieldDescriptor): NativeFieldKind | null {
  const kind = standardComponents[field.component];
  return kind === 'scalar' && modelFields.has(field.name) ? 'model' : (kind ?? null);
}

/** Group adjacent native fields while preserving authored order and custom boundaries. */
export function planNativeFields(fields: readonly NativeFieldDescriptor[]): NativeFieldPlacement[] {
  const pairs = new Set(['width:height', 'sampler_name:scheduler', 'steps:cfg']);
  const result: NativeFieldPlacement[] = [];
  let row = 1;
  for (let index = 0; index < fields.length; index += 1) {
    const field = fields[index];
    if (!field) continue;
    const kind = classifyNativeField(field);
    const next = fields[index + 1];
    const paired =
      kind === 'scalar' &&
      next &&
      classifyNativeField(next) === 'scalar' &&
      pairs.has(`${field.name}:${next.name}`);
    if (kind) result.push({ name: field.name, kind, row, column: 1, span: paired ? 1 : 2 });
    if (paired) {
      result.push({ name: next.name, kind: 'scalar', row, column: 2, span: 1 });
      index += 1;
    }
    row += 1;
  }
  return result;
}
