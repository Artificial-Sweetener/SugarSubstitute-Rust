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
/** Build deterministic invalidation keys for persisted Cube card visibility. */

import { resolveCubeFaceRevealDecision } from './CubeFaceRevealPolicy.js';
import type { ComfyNode } from '../types/graph.js';
import { parseCubeSurfaceState } from './CubeSurfaceState.js';

/**
 * Describe which of a Cube's cards are optional and which of those are revealed. Every internal
 * node contributes its resolved decision, so recording a node's first choice (enabling or disabling
 * it, which stores its defaults beside the choice) changes nothing here.
 */
export function buildCubeCardVisibilitySignature(
  nodes: readonly ComfyNode[],
  surface: unknown,
): string {
  const cards = parseCubeSurfaceState(surface).cards;
  return JSON.stringify(
    nodes
      .map((node) => {
        const id = String(node.id ?? '');
        const decision = resolveCubeFaceRevealDecision(node, cards[id]);
        return [id, decision.authoredBypass, decision.revealed] as const;
      })
      .sort(([leftId], [rightId]) => leftId.localeCompare(rightId)),
  );
}
