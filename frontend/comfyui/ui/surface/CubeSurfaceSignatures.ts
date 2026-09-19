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
/** Derive stable signatures for Cube surface topology and presentation remounts. */

import {
  requireCubeIdentity,
  requireCubeSurface,
  type CubeNode,
} from '../cube/node/ComfyCubeNodeFactory.js';
import { buildCubeCardVisibilitySignature } from './CubeCardVisibilitySignature.js';
import { cubeFaceVisibleWidgets } from './CubeFaceWidgetPolicy.js';

/** Describe internal node identity and the widget ownership that shapes its face card. */
export function buildCubeSurfaceTopologySignature(node: CubeNode): string {
  return JSON.stringify(
    node.subgraph._nodes.map((innerNode) => [
      String(innerNode.id ?? ''),
      innerNode.type ?? '',
      cubeFaceVisibleWidgets(innerNode).map((widget) => widget.name),
    ]),
  );
}

/** Detect titlebar identity and persistence changes without unrelated graph state. */
export function buildCubeSurfacePresentationSignature(node: CubeNode): string {
  const identity = requireCubeIdentity(node);
  return JSON.stringify([
    node.title ?? '',
    node.subgraph.name,
    identity.cube_id ?? '',
    identity.default_alias ?? '',
    identity.cube_version ?? '',
    identity.has_saveable_changes ?? false,
    identity.icon ?? null,
    buildCubeCardVisibilitySignature(requireCubeSurface(node)),
  ]);
}

/** Retry a failed mount only after its renderer-relevant node contract changes. */
export function buildCubeSurfaceMountAttemptSignature(node: CubeNode): string {
  return `${buildCubeSurfaceTopologySignature(node)}|${buildCubeSurfacePresentationSignature(node)}`;
}
