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
/** Adapt one native Cube node into the graph summary consumed by Cube authoring. */
import { requireCubeIdentity } from './ComfyCubeNodeFactory.js';
/** Describe executable nodes and native boundaries without leaking Comfy graph objects. */
export function readCubeNodeAuthoringCandidate(node) {
    const nodes = Array.isArray(node.subgraph._nodes) ? node.subgraph._nodes : [];
    const inputs = Array.isArray(node.subgraph.inputs) ? node.subgraph.inputs : [];
    const outputs = Array.isArray(node.subgraph.outputs) ? node.subgraph.outputs : [];
    return {
        nodeIds: nodes.map((entry, index) => entry && typeof entry === 'object' && 'id' in entry ? entry.id : index),
        markerIds: [...inputs, ...outputs],
        inputCount: inputs.length,
        outputCount: outputs.length,
    };
}
/** Attach the current subgraph summary to titlebar action metadata. */
export function buildCubeFaceChromeMetadata(node) {
    return {
        ...requireCubeIdentity(node),
        graphSummary: readCubeNodeAuthoringCandidate(node),
    };
}
