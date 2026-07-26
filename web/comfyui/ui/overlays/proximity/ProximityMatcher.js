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
/** Match compatible Cube boundary endpoints without owning host discovery. */
import { CubeConnectionTypePolicy, normalizeCubePortType, } from '../../cube/connection/CubeConnectionTypePolicy.js';
import { ProximityInputIndex } from './ProximityInputIndex.js';
/** Own deterministic distance, alias, type, and one-to-one matching policy. */
export class ProximityMatcher {
    #endpoints;
    #compatibility;
    #previousPairs = new Set();
    /** Bind endpoint discovery and Comfy's authoritative type-compatibility API. */
    constructor(endpoints, getLiteGraph, logger) {
        this.#endpoints = endpoints;
        this.#compatibility = new CubeConnectionTypePolicy({ getLiteGraph, logger });
    }
    /** Compute stable nearest compatible matches for all available boundary slots. */
    compute(graph, settings) {
        if (!graph || !Array.isArray(graph._nodes))
            return [];
        const radius = Number(settings.radius) || 160;
        const strict = Boolean(settings.strict);
        const endpoints = this.#endpoints.discover(graph);
        const inputIndex = new ProximityInputIndex(endpoints.inputs);
        const candidates = [];
        for (const output of endpoints.outputs) {
            for (const input of inputIndex.query(output, radius + 40)) {
                if (isSameInstance(output, input))
                    continue;
                if (!output.cube && !input.cube)
                    continue;
                const pairKey = `${output.key}>${input.key}`;
                const releaseRadius = this.#previousPairs.has(pairKey) ? radius + 40 : radius;
                const dx = output.slotPos[0] - input.slotPos[0];
                const dy = output.slotPos[1] - input.slotPos[1];
                const distanceSquared = dx * dx + dy * dy;
                const cubePair = output.cube != null && input.cube != null;
                const adjacency = cubePair
                    ? resolveHorizontalAdjacency(output, input)
                    : Math.sqrt(distanceSquared);
                if (adjacency > releaseRadius ||
                    (cubePair && !hasVerticalAdjacency(output.node, input.node, releaseRadius))) {
                    continue;
                }
                if (!this.#compatibility.accepts(output.type, input.type, strict))
                    continue;
                const aliasMatch = Boolean(output.alias && input.alias && output.alias === input.alias);
                const typeMatch = normalizeCubePortType(output.type) === normalizeCubePortType(input.type);
                candidates.push({
                    output,
                    input,
                    pairKey,
                    cubePair,
                    adjacency,
                    distanceSquared,
                    score: Math.sqrt(distanceSquared) - (aliasMatch ? 40 : 0) - (typeMatch ? 20 : 0),
                });
            }
        }
        candidates.sort(compareCandidates);
        const usedOutputs = new Set();
        const usedInputs = new Set();
        const matches = [];
        const selectedPairs = new Set();
        for (const candidate of candidates) {
            if (usedOutputs.has(candidate.output.key) || usedInputs.has(candidate.input.key)) {
                continue;
            }
            usedOutputs.add(candidate.output.key);
            usedInputs.add(candidate.input.key);
            selectedPairs.add(candidate.pairKey);
            matches.push(toMatch(candidate));
        }
        this.#previousPairs = selectedPairs;
        return matches;
    }
}
/** Prefer adjacent Cube pairs and then preserve canonical slot ordering. */
function compareCandidates(left, right) {
    if (left.cubePair !== right.cubePair)
        return left.cubePair ? -1 : 1;
    if (left.cubePair && right.cubePair) {
        return (left.adjacency - right.adjacency ||
            left.output.slot - right.output.slot ||
            left.input.slot - right.input.slot ||
            left.distanceSquared - right.distanceSquared ||
            left.pairKey.localeCompare(right.pairKey));
    }
    return left.score - right.score || left.pairKey.localeCompare(right.pairKey);
}
/** Convert one selected candidate into overlay and prompt-routing data. */
function toMatch(candidate) {
    const { output, input } = candidate;
    return {
        outputId: output.endpointId,
        outputCube: output.cube,
        outputSlot: output.slot,
        ...(output.node ? { outputNode: output.node } : {}),
        outputPos: output.slotPos,
        outputType: output.type,
        inputId: input.endpointId,
        inputCube: input.cube,
        inputSlot: input.slot,
        inputName: input.slotName,
        ...(input.node ? { inputNode: input.node } : {}),
        inputPos: input.slotPos,
        inputType: input.type,
        originId: output.originId,
        originSlot: output.originSlot,
        promptTargets: input.promptTargets,
        distance: Math.sqrt(candidate.distanceSquared),
        candidateDetails: {
            outType: output.type,
            inType: input.type,
            aliasMatch: Boolean(output.alias && output.alias === input.alias),
            typeMatch: normalizeCubePortType(output.type) === normalizeCubePortType(input.type),
        },
    };
}
/** Exclude a Cube from auto-connecting to another boundary on itself. */
function isSameInstance(output, input) {
    if (output.node && input.node)
        return output.node === input.node;
    return (output.endpointId !== undefined &&
        input.endpointId !== undefined &&
        String(output.endpointId) === String(input.endpointId));
}
/** Measure only the connector-facing horizontal gap for Cube adjacency. */
function resolveHorizontalAdjacency(output, input) {
    const outputRight = readNodeEdge(output.node, 'right', output.slotPos[0]);
    const inputLeft = readNodeEdge(input.node, 'left', input.slotPos[0]);
    if (outputRight > inputLeft)
        return Number.POSITIVE_INFINITY;
    return inputLeft - outputRight;
}
/** Accept vertically overlapping Cubes plus the configured release margin. */
function hasVerticalAdjacency(outputNode, inputNode, margin) {
    if (!outputNode || !inputNode)
        return true;
    const outputTop = readNodeEdge(outputNode, 'top', 0);
    const outputBottom = readNodeEdge(outputNode, 'bottom', outputTop);
    const inputTop = readNodeEdge(inputNode, 'top', 0);
    const inputBottom = readNodeEdge(inputNode, 'bottom', inputTop);
    return Math.max(outputTop, inputTop) <= Math.min(outputBottom, inputBottom) + margin;
}
/** Read one finite node edge with an endpoint-coordinate fallback. */
function readNodeEdge(node, edge, fallback) {
    const x = finite(node?.pos?.[0], fallback);
    const y = finite(node?.pos?.[1], fallback);
    const width = Math.max(0, finite(node?.size?.[0], 0));
    const height = Math.max(0, finite(node?.size?.[1], 0));
    switch (edge) {
        case 'left':
            return x;
        case 'right':
            return x + width;
        case 'top':
            return y;
        case 'bottom':
            return y + height;
    }
}
/** Read one finite host number. */
function finite(value, fallback) {
    const number = Number(value);
    return Number.isFinite(number) ? number : fallback;
}
