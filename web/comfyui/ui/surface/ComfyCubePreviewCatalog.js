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
/** Adapt Comfy's output and live-preview maps into Cube preview media. */
import { readInstanceId } from '../cube/node/CubeNodeCatalog.js';
import { buildCubeOutputExecutionId } from '../cube/execution/CubeOutputExecutionIdentity.js';
import { buildLinkIndex } from '../graph/GraphQuery.js';
import { isRecord } from '../types/common.js';
import { CubePreviewRetentionStore } from './CubePreviewRetentionStore.js';
/** Own Comfy media lookup while leaving preview composition to the surface. */
export class ComfyCubePreviewCatalog {
    #host;
    #lastUnmatchedSignature = new WeakMap();
    #retention;
    #fallbackRootGraph = {};
    #rootGraphScope = null;
    /** Bind Comfy's current media maps and URL adapter. */
    constructor(host) {
        this.#host = host;
        this.#retention = host.retention ?? new CubePreviewRetentionStore();
    }
    /** Collect selected-output candidates and suppressed internal previews. */
    snapshot(cube) {
        const instanceId = readInstanceId(cube);
        const locatedNodes = locateInternalNodes(cube);
        const locatorByNode = new Map(locatedNodes.map(({ node, locator }) => [node, locator]));
        const outputItems = new Set();
        const outputs = cube.subgraph.outputs.map((output, index) => {
            const id = readString(output.name) || String(index);
            const executionId = buildCubeOutputExecutionId(cube.id, index);
            const capturedItems = this.#readOutputItems(this.#host.getCubeOutput?.(executionId), executionId, id);
            const markerItems = capturedItems.length > 0 ? capturedItems : this.#readNodeItems(executionId, id);
            const source = resolveOutputNode(cube, index);
            const locator = source ? (locatorByNode.get(source) ?? '') : '';
            const directItems = source && locator ? this.#readNodeItems(locator, id) : [];
            const items = markerItems.length > 0
                ? markerItems
                : directItems.length > 0
                    ? directItems
                    : this.#readDownstreamOutputItems(cube, index, id);
            for (const item of items)
                outputItems.add(item.url);
            return { id, label: id, items };
        });
        const internalItems = locatedNodes
            .flatMap(({ node, locator }) => {
            const label = readString(node.title) || readString(node.type) || 'Internal node';
            return this.#readNodeItems(locator, label);
        })
            .filter((item) => !outputItems.has(item.url));
        const current = { outputs, internalItems: deduplicateItems(internalItems) };
        const outputSignature = outputs.map((output) => output.id).join('\u0000');
        this.#rootGraphScope ??= this.#host.getRootGraph?.() ?? this.#fallbackRootGraph;
        const rootGraph = this.#rootGraphScope;
        const hasMedia = outputs.some((output) => output.items.length > 0) || current.internalItems.length > 0;
        if (hasMedia) {
            this.#retention.retain(rootGraph, instanceId, outputSignature, current);
        }
        const retained = this.#retention.read(rootGraph, instanceId, outputSignature);
        const snapshot = !hasMedia && retained ? retained : current;
        this.#reportUnmatchedMedia(cube, locatedNodes, snapshot);
        return snapshot;
    }
    /** Resolve final Comfy media from the nearest consumer of one Cube output. */
    #readDownstreamOutputItems(cube, outputSlot, label) {
        const graph = this.#host.getRootGraph?.() ?? cube.graph;
        if (!graph)
            return [];
        const links = buildLinkIndex(graph);
        const effectiveLinks = indexEffectiveLinks(this.#host.getEffectiveLinks?.() ?? []);
        let frontier = [
            ...(links.outgoing.get(String(cube.id)) ?? [])
                .filter((link) => readSlot(link.origin_slot) === outputSlot)
                .map((link) => readNodeId(link.target_id ?? link.target)),
            ...(effectiveLinks.get(String(cube.id)) ?? [])
                .filter((link) => link.originSlot === outputSlot)
                .map((link) => readNodeId(link.targetId)),
        ].filter((id) => id !== null);
        const visited = new Set([String(cube.id)]);
        while (frontier.length > 0) {
            const nextFrontier = [];
            const media = [];
            for (const nodeId of frontier) {
                if (visited.has(nodeId))
                    continue;
                visited.add(nodeId);
                media.push(...this.#readNodeItems(nodeId, label));
                for (const link of links.outgoing.get(nodeId) ?? []) {
                    const targetId = readNodeId(link.target_id ?? link.target);
                    if (targetId && !visited.has(targetId))
                        nextFrontier.push(targetId);
                }
                for (const link of effectiveLinks.get(nodeId) ?? []) {
                    const targetId = readNodeId(link.targetId);
                    if (targetId && !visited.has(targetId))
                        nextFrontier.push(targetId);
                }
            }
            if (media.length > 0)
                return deduplicateItems(media);
            frontier = nextFrontier;
        }
        return [];
    }
    /** Report a changed host-media inventory when none belongs to this Cube. */
    #reportUnmatchedMedia(cube, locatedNodes, snapshot) {
        if (!this.#host.logger)
            return;
        if (snapshot.outputs.some((output) => output.items.length > 0) ||
            snapshot.internalItems.length > 0) {
            this.#lastUnmatchedSignature.delete(cube);
            return;
        }
        const outputKeys = Object.keys(this.#host.getNodeOutputs()).sort();
        const previewKeys = Object.keys(this.#host.getNodePreviewImages()).sort();
        if (outputKeys.length === 0 && previewKeys.length === 0)
            return;
        const expectedLocators = locatedNodes.map(({ locator }) => locator).sort();
        const signature = JSON.stringify([String(cube.id), outputKeys, previewKeys, expectedLocators]);
        if (signature === this.#lastUnmatchedSignature.get(cube))
            return;
        this.#lastUnmatchedSignature.set(cube, signature);
        this.#host.logger.debug('SugarCubes found no Cube preview for the current Comfy media inventory.', JSON.stringify({
            cubeNodeId: cube.id,
            expectedLocators,
            outputKeys,
            previewKeys,
        }));
    }
    /** Read durable execution media before renderer-owned transient previews. */
    #readNodeItems(locator, label) {
        const output = this.#host.getNodeOutputs()[locator];
        const outputItems = this.#readOutputItems(output, locator, label);
        if (outputItems.length > 0)
            return outputItems;
        const livePreviews = this.#host.getNodePreviewImages()[locator];
        if (!Array.isArray(livePreviews))
            return [];
        return livePreviews
            .filter((value) => typeof value === 'string' && value.length > 0)
            .map((url, index) => ({
            key: `${locator}:preview:${index}`,
            url: stabilizeComfyViewUrl(url),
            label,
            sourceLocator: locator,
        }));
    }
    /** Convert one durable execution payload into preview items. */
    #readOutputItems(output, locator, label) {
        if (isRecord(output) && Array.isArray(output.images)) {
            const items = output.images
                .map((image, index) => {
                if (!isRecord(image))
                    return null;
                const builtUrl = this.#host.buildOutputImageUrl(image);
                const url = builtUrl ? stabilizeComfyViewUrl(builtUrl) : null;
                return url
                    ? {
                        key: `${locator}:output:${index}`,
                        url,
                        label,
                        sourceLocator: locator,
                    }
                    : null;
            })
                .filter((item) => item !== null);
            if (items.length > 0)
                return items;
        }
        return [];
    }
}
/** Locate internal nodes by the frontend identities used by Comfy's output store. */
function locateInternalNodes(cube) {
    const located = [];
    appendLocatedNodes(cube.subgraph, [cube.id], located);
    return located;
}
/** Resolve one Cube output through Comfy's native subgraph boundary objects. */
function resolveOutputNode(cube, slot) {
    const outputNode = isRecord(cube.subgraph.outputNode) ? cube.subgraph.outputNode : null;
    const slots = Array.isArray(outputNode?.slots) ? outputNode.slots : [];
    const outputSlot = isRecord(slots[slot]) ? slots[slot] : null;
    const getLinks = outputSlot?.getLinks;
    if (typeof getLinks !== 'function')
        return undefined;
    const links = getLinks.call(outputSlot);
    const link = Array.isArray(links) ? links[0] : undefined;
    if (!isRecord(link) || typeof link.resolve !== 'function')
        return undefined;
    const resolved = link.resolve(cube.subgraph);
    return isRecord(resolved) && isRecord(resolved.outputNode)
        ? resolved.outputNode
        : undefined;
}
/**
 * Traverse nested graphs using Comfy's flattened SubgraphNode instance path.
 *
 * Backend execution IDs and frontend output-map keys both retain the complete
 * root-to-leaf SubgraphNode path.
 */
function appendLocatedNodes(graph, executionPath, located) {
    for (const node of graph._nodes) {
        const localNodeId = String(node.id ?? '');
        if (!localNodeId)
            continue;
        const nodePath = [...executionPath, localNodeId];
        const locator = nodePath.map(String).join(':');
        located.push({ node, locator });
        const subgraph = isRecord(node.subgraph) ? node.subgraph : null;
        if (subgraph && Array.isArray(subgraph._nodes)) {
            appendLocatedNodes(subgraph, nodePath, located);
        }
    }
}
/** Read one trimmed external string. */
function readString(value) {
    return typeof value === 'string' ? value.trim() : '';
}
/** Read one exact non-negative slot index. */
function readSlot(value) {
    const slot = Number(value);
    return Number.isInteger(slot) && slot >= 0 ? slot : null;
}
/** Read one finite graph-node identity. */
function readNodeId(value) {
    if (typeof value === 'string')
        return value.trim() || null;
    return typeof value === 'number' && Number.isFinite(value) ? String(value) : null;
}
/** Group prompt-effective dotted routes by their real source node. */
function indexEffectiveLinks(links) {
    const indexed = new Map();
    for (const link of links) {
        const originId = readNodeId(link.originId);
        const targetId = readNodeId(link.targetId);
        if (!originId || !targetId || readSlot(link.originSlot) === null)
            continue;
        const outgoing = indexed.get(originId) ?? [];
        outgoing.push(link);
        indexed.set(originId, outgoing);
    }
    return indexed;
}
/** Preserve item order while removing duplicate media URLs. */
function deduplicateItems(items) {
    const seen = new Set();
    return items.filter((item) => {
        if (seen.has(item.url))
            return false;
        seen.add(item.url);
        return true;
    });
}
/** Remove volatile Comfy view tokens while retaining the media's stable identity. */
function stabilizeComfyViewUrl(value) {
    const base = 'http://sugarcubes.invalid';
    try {
        const url = new URL(value, base);
        if (url.pathname !== '/view' && url.pathname !== '/api/view')
            return value;
        url.searchParams.delete('rand');
        return url.origin === base ? `${url.pathname}${url.search}${url.hash}` : url.href;
    }
    catch {
        return value;
    }
}
