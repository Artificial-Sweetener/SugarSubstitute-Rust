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
/** Resolve editable Cube-boundary controls without moving their value ownership. */
import type { CubeNode } from '../../cube/node/ComfyCubeNodeFactory.js';
import type { ComfyNode } from '../../types/graph.js';
import type { NativeNodeWidgetSource } from '../NativeNodeCardRenderer.js';

/** Forward exposed widgets into their internal node cards while retaining host identity. */
export function resolveMasonryWidgetSources(
  cube: CubeNode,
  node: ComfyNode,
): NativeNodeWidgetSource[] {
  const sources: NativeNodeWidgetSource[] = [];
  for (const input of node.inputs ?? []) {
    if (input.link == null || !input.widget?.name) continue;
    const link = cube.subgraph.getLink?.(input.link);
    if (!link || String(link.origin_id) !== String(cube.subgraph.inputNode.id)) continue;
    if (typeof link.origin_slot !== 'number') continue;
    const boundary = cube.inputs?.[link.origin_slot];
    const name = boundary?.widget?.name;
    if (!name || boundary.link != null || (boundary.links?.length ?? 0) > 0) continue;
    sources.push({ node: cube, name, targetName: input.widget.name });
  }
  return sources;
}
