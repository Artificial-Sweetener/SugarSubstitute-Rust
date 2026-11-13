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
/** Define Substitute-compatible version-one records independently of browser and persistence. */
import type { DimensionShape } from './DimensionPolicy.js';
export type PresetScope =
  | 'global'
  | 'model_family'
  | 'provider_model'
  | 'provider_model_version'
  | 'local_model';
export type PresetValue =
  | null
  | string
  | number
  | boolean
  | PresetValue[]
  | { [key: string]: PresetValue };
export type PresetInputs = Readonly<Record<string, PresetValue>>;
export interface PresetAssociation {
  readonly scope: PresetScope;
  readonly provider: string | null;
  readonly key: string;
  readonly label: string;
}
interface PresetRecord {
  readonly id: string;
  readonly label: string;
  readonly associations: readonly PresetAssociation[];
  readonly created_at: string;
  readonly updated_at: string;
}
export type UserPreset = PresetRecord &
  (
    | { readonly kind: 'dimension'; readonly payload: DimensionShape }
    | {
        readonly kind: 'node_inputs';
        readonly payload: { readonly node_type: string; readonly inputs: PresetInputs };
      }
    | { readonly kind: 'prompt_string'; readonly payload: { readonly text: string } }
  );
/** Preserve the shared library's global association identity. */
export const GLOBAL_ASSOCIATION: PresetAssociation = {
  scope: 'global',
  provider: null,
  key: 'global',
  label: 'Global',
};
/** Compare persisted identities rather than translated presentation labels. */
export function sameAssociation(left: PresetAssociation, right: PresetAssociation): boolean {
  return left.scope === right.scope && left.provider === right.provider && left.key === right.key;
}
