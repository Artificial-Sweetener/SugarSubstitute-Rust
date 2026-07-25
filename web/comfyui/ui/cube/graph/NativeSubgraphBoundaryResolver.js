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
/** Resolve native subgraph boundaries to Comfy's flattened prompt identities. */
import { getGraphNodes } from '../../graph/GraphQuery.js';
import { isRecord } from '../../types/common.js';
/** Own the one recursive translation from surface slots to prompt endpoints. */
export class NativeSubgraphBoundaryResolver {
    #logger;
    /** Bind diagnostics for malformed or circular host subgraphs. */
    constructor(logger) {
        this.#logger = logger;
    }
    /** Resolve one surface output to the leaf output used by Comfy prompt flattening. */
    resolveOutput(node, slot) {
        if (node.id == null)
            return null;
        return this.#resolveOutput(node, slot, [node.id], new Set());
    }
    /** Resolve one surface input to every leaf prompt target behind its boundary. */
    resolveInputTargets(node, slot) {
        if (node.id == null)
            return [];
        return this.#resolveInputTargets(node, slot, [node.id], new Set());
    }
    /** Recurse through output boundaries while retaining the exact instance path. */
    #resolveOutput(node, slot, executionPath, visited) {
        if (!isTraversableSubgraphNode(node)) {
            return { nodeId: joinExecutionPath(executionPath), slot };
        }
        const visitKey = `${executionPath.map(String).join(':')}[O]${String(slot)}`;
        if (visited.has(visitKey)) {
            this.#logger.warn('SugarCubes skipped a circular native subgraph output boundary.', {
                nodeId: node.id,
                slot,
            });
            return null;
        }
        const nextVisited = new Set(visited);
        nextVisited.add(visitKey);
        const resolved = resolveSubgraphOutput(node, slot);
        const outputNode = resolved?.outputNode;
        const outputSlot = readLinkSlot(resolved?.link?.origin_slot, outputNode?.outputs, resolved?.output);
        if (!outputNode || outputSlot < 0 || outputNode.id == null)
            return null;
        return this.#resolveOutput(outputNode, outputSlot, [...executionPath, outputNode.id], nextVisited);
    }
    /** Recurse through input fanout while retaining every exact instance path. */
    #resolveInputTargets(node, slot, executionPath, visited) {
        if (!isTraversableSubgraphNode(node)) {
            const input = node.inputs?.[slot];
            if (!input)
                return [];
            return [
                {
                    nodeId: joinExecutionPath(executionPath),
                    inputSlot: slot,
                    inputName: readSlotName(input, slot),
                },
            ];
        }
        const visitKey = `${executionPath.map(String).join(':')}[I]${String(slot)}`;
        if (visited.has(visitKey)) {
            this.#logger.warn('SugarCubes skipped a circular native subgraph input boundary.', {
                nodeId: node.id,
                slot,
            });
            return [];
        }
        const nextVisited = new Set(visited);
        nextVisited.add(visitKey);
        const targets = [];
        for (const resolved of resolveSubgraphInputs(node, slot)) {
            const inputNode = resolved.inputNode;
            const inputSlot = readLinkSlot(resolved.link?.target_slot, inputNode?.inputs, resolved.input);
            if (!inputNode || inputSlot < 0 || inputNode.id == null)
                continue;
            targets.push(...this.#resolveInputTargets(inputNode, inputSlot, [...executionPath, inputNode.id], nextVisited));
        }
        return dedupeInputTargets(targets);
    }
}
/** Resolve Comfy's authoritative subgraph input links with a version-safe fallback. */
function resolveSubgraphInputs(node, slot) {
    if (typeof node.resolveSubgraphInputLinks === 'function') {
        return node.resolveSubgraphInputLinks(slot);
    }
    const boundary = node.subgraph.inputNode?.slots?.[slot];
    return (boundary?.linkIds ?? [])
        .map((linkId) => resolveConnection(node.subgraph, linkId))
        .filter((value) => value !== null);
}
/** Resolve Comfy's authoritative subgraph output link with a version-safe fallback. */
function resolveSubgraphOutput(node, slot) {
    if (typeof node.resolveSubgraphOutputLink === 'function') {
        return node.resolveSubgraphOutputLink(slot) ?? null;
    }
    const linkId = node.subgraph.outputNode?.slots?.[slot]?.linkIds?.[0];
    return linkId == null ? null : resolveConnection(node.subgraph, linkId);
}
/** Resolve one boundary link without requiring Comfy's concrete LLink class. */
function resolveConnection(graph, linkId) {
    const link = readGraphLink(graph, linkId);
    if (!link)
        return null;
    const inputNode = readGraphNode(graph, link.target_id ?? link.target);
    const outputNode = readGraphNode(graph, link.origin_id ?? link.origin);
    const inputSlot = Number(link.target_slot);
    const outputSlot = Number(link.origin_slot);
    const input = inputNode?.inputs?.[inputSlot];
    const output = outputNode?.outputs?.[outputSlot];
    return {
        link,
        ...(inputNode ? { inputNode } : {}),
        ...(input ? { input } : {}),
        ...(outputNode ? { outputNode } : {}),
        ...(output ? { output } : {}),
    };
}
/** Read one graph link across Comfy's public and serialized containers. */
function readGraphLink(graph, linkId) {
    if (typeof graph.getLink === 'function')
        return graph.getLink(linkId) ?? null;
    for (const container of [graph._links, graph.links]) {
        if (container instanceof Map) {
            const value = container.get(linkId);
            if (value)
                return value;
        }
        else if (Array.isArray(container)) {
            const value = container.find((entry) => String(entry.id) === String(linkId));
            if (value)
                return value;
        }
        else if (isRecord(container)) {
            const value = container[String(linkId)];
            if (isRecord(value))
                return value;
        }
    }
    return null;
}
/** Read one real graph node without assuming a concrete LGraph implementation. */
function readGraphNode(graph, id) {
    if (id == null)
        return null;
    if (typeof graph.getNodeById === 'function') {
        const value = graph.getNodeById.call(graph, id);
        if (isRecord(value))
            return value;
    }
    return getGraphNodes(graph).find((node) => String(node.id) === String(id)) ?? null;
}
/** Identify a real Comfy subgraph host without depending on its concrete class. */
function isTraversableSubgraphNode(node) {
    return (typeof node.isSubgraphNode === 'function' &&
        node.isSubgraphNode() === true &&
        isRecord(node.subgraph));
}
/** Resolve a slot index from the link first and the exact slot object second. */
function readLinkSlot(value, slots, slot) {
    const index = Number(value);
    if (Number.isInteger(index) && index >= 0)
        return index;
    return slot && slots ? slots.indexOf(slot) : -1;
}
/** Read a stable input name even when a host node omits one. */
function readSlotName(slot, index) {
    for (const value of [slot.name, slot.label]) {
        if (typeof value === 'string' && value.trim())
            return value.trim();
    }
    return `input.${String(index)}`;
}
/** Join the exact flattened execution path used by Comfy subgraphs. */
function joinExecutionPath(path) {
    return path.map(String).join(':');
}
/** Remove duplicate leaf targets created by fanout through nested boundaries. */
function dedupeInputTargets(targets) {
    const seen = new Set();
    return targets.filter((target) => {
        const key = `${String(target.nodeId)}:${String(target.inputSlot)}:${target.inputName}`;
        if (seen.has(key))
            return false;
        seen.add(key);
        return true;
    });
}
