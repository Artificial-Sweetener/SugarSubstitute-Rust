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
/** Project one authoritative set of native cards for Cube faces and workflow masonry. */
import type { ComfyGraph, ComfyNode } from '../types/graph.js';
import type { CubeNode } from '../cube/node/ComfyCubeNodeFactory.js';
import {
  resolveCubeFaceCardPresentation,
  type CubeFaceCardDecision,
} from './CubeFaceCardPolicy.js';
import type { CubeSurfaceState } from './CubeSurfaceState.js';
import type { NativeNodeWidgetSource } from './NativeNodeCardRenderer.js';
import { resolveCubeCardWidgetSources } from './CubeCardWidgetSources.js';
import { cubeFaceCardColumnSpan, findCubeFacePromptWidget } from './CubeFacePromptPolicy.js';

export interface ProjectedNodeCard extends CubeFaceCardDecision {
  prompt?: boolean;
  widgetSources?: readonly NativeNodeWidgetSource[];
}

/** Keep exposed controls, prompt spans and reading order identical across both surfaces. */
export function projectNodeCards(
  nodes: readonly ComfyNode[],
  state: CubeSurfaceState,
  graph?: ComfyGraph,
  cube?: CubeNode,
): ProjectedNodeCard[] {
  const sources = new Map(
    nodes.map((node) => [node, cube ? resolveCubeCardWidgetSources(cube, node) : []]),
  );
  const additionalControlNodes = new Set(
    nodes.filter((node) => (sources.get(node)?.length ?? 0) > 0),
  );
  return resolveCubeFaceCardPresentation(nodes, state, graph, additionalControlNodes).cards.map(
    (card) => {
      const widgetSources = sources.get(card.node) ?? [];
      const exposed = new Set(widgetSources.map((source) => source.targetName));
      const presentedNode = {
        ...card.node,
        inputs: (card.node.inputs ?? []).filter((input) => !exposed.has(input.widget?.name ?? '')),
      };
      return {
        ...card,
        widgetSources,
        prompt: findCubeFacePromptWidget(presentedNode) !== null,
        columnSpan: cubeFaceCardColumnSpan(presentedNode),
      };
    },
  );
}
