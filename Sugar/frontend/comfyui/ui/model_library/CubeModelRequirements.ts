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
/** Assign portable Cube model policy through native identity and definition owners. */
import { isCubeNode, requireCubeIdentity } from '../cube/node/ComfyCubeNodeFactory.js';
import { writeCubeDefinitionIdentity } from '../cube/node/CubeDefinitionIdentityWriter.js';
import { readCubeModelRequirement } from '../cube/node/CubeNodeInstanceCatalog.js';
import { locateModelGraph } from './ComfyModelGraph.js';
import { encodeModelRequirement, type ModelRequirement } from '../cube/ModelRequirement.js';
import type { ModelPickerBinding } from './ModelPickerBinding.js';

/** Read explicit assignments separately from legacy Cube target labels. */
export function cubeModelRequirement(
  app: unknown,
  binding: ModelPickerBinding,
): ModelRequirement | null {
  const owner = locateModelGraph(app, binding)?.scope.owner;
  if (!isCubeNode(owner)) return null;
  return readCubeModelRequirement(owner);
}

/** Mutate only a Cube-owned requirement and retain its native dirty/serialization lifecycle. */
export function assignCubeModelRequirement(
  app: unknown,
  binding: ModelPickerBinding,
  requirement: ModelRequirement,
): void {
  const owner = locateModelGraph(app, binding)?.scope.owner;
  if (!isCubeNode(owner)) throw new Error('The model field is not owned by a SugarCube.');
  owner.graph?.beforeChange?.();
  const identity = requireCubeIdentity(owner);
  identity.model_requirements = encodeModelRequirement(requirement);
  writeCubeDefinitionIdentity(owner.subgraph, 'cube', identity);
  owner.graph?.afterChange?.();
  owner.graph?.setDirtyCanvas?.(true, true);
}
