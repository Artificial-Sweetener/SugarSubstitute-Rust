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
/** Translate host-neutral picker descriptors into Comfy V1 node definitions. */

import type { CubePickerDescriptor } from './CubePickerDescriptor.js';
import { resolveCubePackIdentity } from '../core/CubePackIdentity.js';

/** Reserve one collision-resistant namespace for picker-only Cube definitions. */
export const CUBE_NODE_TYPE_PREFIX = 'SugarCubes.Cube.';
/** Advertise Cubes as their own truthful top-level Comfy category. */
export const CUBE_NODE_CATEGORY = 'SugarCubes';
/** Keep legacy Cubes discoverable without pretending they target a known model. */
export const CUBE_UNSPECIFIED_MODEL_CATEGORY = 'Unspecified';

export interface ComfyCubeInputOptions {
  forceInput: true;
  display_name: string;
}

export type ComfyCubeInputSpec = readonly [string, ComfyCubeInputOptions];

export interface ComfyCubeNodeDefinition {
  name: string;
  display_name: string;
  description: string;
  category: string;
  python_module: string;
  sugarcubes_pack_name: string;
  sugarcubes_target_model: string;
  output_node: false;
  input: { required: Record<string, ComfyCubeInputSpec> };
  input_order: { required: string[] };
  output: string[];
  output_name: string[];
  output_is_list: false[];
  search_aliases: string[];
}

/** Build one native-search definition whose interface is canonical boundaries only. */
export function projectComfyCubeNodeDef(descriptor: CubePickerDescriptor): ComfyCubeNodeDefinition {
  const required: Record<string, ComfyCubeInputSpec> = {};
  for (const boundary of descriptor.inputs) {
    required[boundary.name] = [boundary.type, { forceInput: true, display_name: boundary.label }];
  }
  const targetModel = descriptor.targetModel || CUBE_UNSPECIFIED_MODEL_CATEGORY;
  const pack = resolveCubePackIdentity(descriptor);
  return {
    name: `${CUBE_NODE_TYPE_PREFIX}${descriptor.key}`,
    display_name: descriptor.displayName,
    description: descriptor.description,
    category: `${CUBE_NODE_CATEGORY}/${targetModel}`,
    python_module: `custom_nodes.${pack.label}`,
    sugarcubes_pack_name: pack.label,
    sugarcubes_target_model: targetModel,
    output_node: false,
    input: { required },
    input_order: { required: descriptor.inputs.map((boundary) => boundary.name) },
    output: descriptor.outputs.map((boundary) => boundary.type),
    output_name: descriptor.outputs.map((boundary) => boundary.label),
    output_is_list: descriptor.outputs.map(() => false),
    search_aliases: [...descriptor.searchTerms],
  };
}

/** Identify the namespace reserved for picker-created Cube definitions. */
export function isComfyCubePickerType(value: unknown): value is string {
  return typeof value === 'string' && value.startsWith(CUBE_NODE_TYPE_PREFIX);
}
