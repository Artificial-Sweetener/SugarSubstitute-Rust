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
/** Resolve explicit cube-family ownership for native model fields without filename heuristics. */

import { isRecord } from '../types/common.js';
import type { CubeNode } from '../cube/node/ComfyCubeNodeFactory.js';
import { readCubeTargetModel } from '../cube/node/CubeNodeInstanceCatalog.js';
import type { ModelPickerBinding } from './ModelPickerBinding.js';
import { isScopedModelKind } from './ModelPickerCompatibilityScope.js';

/** Require unambiguous graph scope before attributing an internal field to a cube's model family. */
export function comfyModelTarget(binding: ModelPickerBinding, cubes: readonly CubeNode[]): string {
  if (!isScopedModelKind(binding.kind)) return '';
  const locator =
    binding.nodeLocator ??
    binding.anchor.closest<HTMLElement>('[data-cube-node-locator]')?.dataset.cubeNodeLocator;
  if (!locator) return '';
  const owners = cubes.filter((cube) => containsLocator(cube.subgraph, locator, new Set()));
  return owners.length === 1 && owners[0] ? readCubeTargetModel(owners[0]) : '';
}

/** Follow nested graph scopes while rejecting cycles and unscoped numeric node identities. */
function containsLocator(value: unknown, locator: string, seen: Set<object>): boolean {
  if (!isRecord(value) || seen.has(value)) return false;
  seen.add(value);
  if (Array.isArray(value._nodes)) {
    for (const node of value._nodes) {
      if (!isRecord(node)) continue;
      if (`${String(value.id)}:${String(node.id)}` === locator) return true;
      if (containsLocator(node.subgraph, locator, seen)) return true;
    }
  }
  if (value._subgraphs instanceof Map)
    for (const graph of value._subgraphs.values())
      if (containsLocator(graph, locator, seen)) return true;
  return false;
}
