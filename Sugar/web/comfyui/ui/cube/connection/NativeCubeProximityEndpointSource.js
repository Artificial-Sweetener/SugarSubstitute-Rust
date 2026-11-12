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
/** Project native graph and subgraph slots into transient proximity endpoints. */
import { getGraphNodes } from '../../graph/GraphQuery.js';
import { ComfyGraphGeometry } from '../../overlays/proximity/ComfyGraphGeometry.js';
import { isRecord } from '../../types/common.js';
import { isCubeNode, requireCubeIdentity } from '../node/ComfyCubeNodeFactory.js';
/** Own endpoint discovery across native root nodes and recursively flattened subgraphs. */
export class NativeCubeProximityEndpointSource {
    #geometry;
    #logger;
    #boundaryResolver;
    #portPresentation;
    #lastDiscoverySignature = '';
    /** Bind native graph geometry and actionable traversal diagnostics. */
    constructor(logger, boundaryResolver, portPresentation = null) {
        this.#logger = logger;
        this.#geometry = new ComfyGraphGeometry(logger);
        this.#boundaryResolver = boundaryResolver;
        this.#portPresentation = portPresentation;
    }
    /** Discover every currently unlinked root slot that can participate with a Cube. */
    discover(graphValue) {
        if (!isRecord(graphValue))
            return { outputs: [], inputs: [] };
        const graph = graphValue;
        const outputs = [];
        const inputs = [];
        for (const node of getGraphNodes(graph)) {
            if (node.id == null)
                continue;
            const identity = readEndpointIdentity(node);
            for (let slot = 0; slot < (node.outputs?.length ?? 0); slot += 1) {
                const output = node.outputs?.[slot];
                if (!output || hasLiveLink(output))
                    continue;
                const origin = this.#boundaryResolver.resolveOutput(node, slot);
                if (!origin)
                    continue;
                const slotName = readSlotName(output, slot, 'output');
                outputs.push({
                    key: `${String(node.id)}:output:${slot}`,
                    endpointId: node.id,
                    node,
                    slot,
                    cube: identity.cube,
                    instanceId: identity.instanceId,
                    alias: readAlias(slotName),
                    type: output.type,
                    slotPos: this.#resolveStablePosition(node, true, slot),
                    slotName,
                    originId: origin.nodeId,
                    originSlot: origin.slot,
                });
            }
            for (let slot = 0; slot < (node.inputs?.length ?? 0); slot += 1) {
                const input = node.inputs?.[slot];
                if (!input || hasLiveLink(input))
                    continue;
                const promptTargets = this.#boundaryResolver.resolveInputTargets(node, slot);
                if (!promptTargets.length)
                    continue;
                const slotName = readSlotName(input, slot, 'input');
                inputs.push({
                    key: `${String(node.id)}:input:${slot}`,
                    endpointId: node.id,
                    node,
                    slot,
                    cube: identity.cube,
                    instanceId: identity.instanceId,
                    alias: readAlias(slotName),
                    type: input.type,
                    slotPos: this.#resolveStablePosition(node, false, slot),
                    slotName,
                    promptTargets,
                });
            }
        }
        this.#reportChangedInventory(outputs, inputs);
        return { outputs, inputs };
    }
    /** Match from settled targets so animation cannot feed back or detach during resize. */
    #resolveStablePosition(node, isOutput, slot) {
        const fallback = this.#geometry.slotPosition(node, isOutput, slot);
        return (this.#portPresentation?.resolveMatchingGraphPosition(node, isOutput ? 'output' : 'input', slot, fallback) ?? fallback);
    }
    /** Log endpoint ownership only when graph mutations change the inventory. */
    #reportChangedInventory(outputs, inputs) {
        const signature = [
            ...outputs.map((endpoint) => `${endpoint.key}->${String(endpoint.originId)}`),
            ...inputs.map((endpoint) => {
                const targets = endpoint.promptTargets.map((target) => String(target.nodeId)).join(',');
                return `${endpoint.key}->${targets}`;
            }),
        ].join('|');
        if (signature === this.#lastDiscoverySignature)
            return;
        this.#lastDiscoverySignature = signature;
        const positions = [
            ...outputs.map((endpoint) => `${endpoint.key}@${formatPoint(endpoint.slotPos)}`),
            ...inputs.map((endpoint) => `${endpoint.key}@${formatPoint(endpoint.slotPos)}`),
        ].join(';');
        this.#logger.debug(`SugarCubes discovered ${String(outputs.length)} output and ` +
            `${String(inputs.length)} input proximity endpoints` +
            `${positions ? ` at ${positions}` : ''}.`, {
            outputs: outputs.length,
            inputs: inputs.length,
            cubeOutputs: outputs.filter((endpoint) => endpoint.cube != null).length,
            cubeInputs: inputs.filter((endpoint) => endpoint.cube != null).length,
        });
    }
}
/** Read durable Cube identity while keeping ordinary nodes eligible as counterparts. */
function readEndpointIdentity(node) {
    if (!isCubeNode(node)) {
        return { cube: null, instanceId: String(node.id ?? '') };
    }
    const identity = requireCubeIdentity(node);
    const cube = readString(identity.cube_id) ??
        readString(identity.definition_id) ??
        readString(node.subgraph.id);
    const instanceId = readString(identity.instance_id) ?? String(node.id);
    return { cube, instanceId };
}
/** Exclude slots already owned by ordinary persisted Comfy links. */
function hasLiveLink(slot) {
    const links = Array.isArray(slot.links) ? slot.links : [];
    return slot.link != null || links.some((linkId) => linkId != null);
}
/** Read a stable slot name even when a host node omits one. */
function readSlotName(slot, index, direction) {
    return readString(slot.name) ?? readString(slot.label) ?? `${direction}.${String(index)}`;
}
/** Normalize Cube boundary prefixes for stronger input/output alias matching. */
function readAlias(name) {
    const segments = name.split('.').filter(Boolean);
    return (segments.at(-1) ?? name).trim().toLowerCase();
}
/** Read one non-empty external string. */
function readString(value) {
    return typeof value === 'string' && value.trim() ? value.trim() : null;
}
/** Format one graph-space point for actionable endpoint diagnostics. */
function formatPoint(point) {
    return point.map((value) => String(Math.round(value))).join(',');
}
