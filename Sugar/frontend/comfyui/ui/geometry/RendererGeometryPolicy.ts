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
/** Own renderer-specific geometry policy at the ComfyUI host boundary. */

export type NodeRenderer = 'litegraph' | 'vue';

export interface RendererGeometryPolicy {
  renderer: NodeRenderer;
  minimumExpandedNodeWidth: number;
}

export interface RendererGeometryHost {
  vueNodesMode?: boolean;
}

const NODES_TWO_MINIMUM_WIDTH = 225;

/** Resolve the active renderer's geometry rules from public LiteGraph state. */
export function resolveRendererGeometryPolicy(
  liteGraph: RendererGeometryHost | null | undefined,
  rendererOverride?: NodeRenderer,
): RendererGeometryPolicy {
  const renderer = rendererOverride ?? (liteGraph?.vueNodesMode === true ? 'vue' : 'litegraph');
  return {
    renderer,
    minimumExpandedNodeWidth: renderer === 'vue' ? NODES_TWO_MINIMUM_WIDTH : 0,
  };
}
