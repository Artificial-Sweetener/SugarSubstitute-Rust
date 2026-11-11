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
/** Project accepted proximity matches into shared workflow ordering edges. */
import type { ComfyGraph, GraphId } from '../../types/graph.js';
import type { ProximityOverlay } from '../ProximityOverlay.js';

/** Reuse execution acceptance when exposing implicit dependencies to workflow features. */
export function proximityWorkflowConnections(
  proximity: Pick<ProximityOverlay, 'resolveExecutionMatches'>,
  graph: ComfyGraph,
): readonly { source: GraphId; target: GraphId }[] {
  return proximity
    .resolveExecutionMatches(graph)
    .flatMap((match) =>
      match.outputId === undefined || match.inputId === undefined
        ? []
        : [{ source: match.outputId, target: match.inputId }],
    );
}
