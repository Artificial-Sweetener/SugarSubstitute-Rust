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
/** Read the view a workflow document asks to open in, and the default when it names none. */
import { isRecord } from '../../types/common.js';
import { isCubeKind } from '../../cube/CubeKind.js';
/** The key in a workflow's root `extra` that records whether it was showing masonry. */
export const WORKFLOW_VIEW_EXTRA = 'sugarcubes_view';
/**
 * Whether a serialized workflow opens in masonry: the view it records, or, when it records none,
 * masonry exactly when its root graph holds a Cube.
 */
export function workflowOpensInMasonry(graph) {
    if (!isRecord(graph))
        return false;
    const recorded = isRecord(graph.extra) ? graph.extra[WORKFLOW_VIEW_EXTRA] : undefined;
    if (recorded === 'masonry' || recorded === 'graph')
        return recorded === 'masonry';
    return (Array.isArray(graph.nodes) &&
        graph.nodes.some((node) => isRecord(node) && isRecord(node.properties) && isCubeKind(node.properties.sugarcubes_kind)));
}
