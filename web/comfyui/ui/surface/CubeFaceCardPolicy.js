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
/** Resolve renderer-neutral Cube-face card visibility and activation controls. */
import { cubeFaceNodeHasVisibleWidgets } from './CubeFaceNodePresentationPolicy.js';
import { cubeFaceNodeAllowsActivationControl, inferCubeFaceTransformSignals, isHardHiddenCubeFaceNode, } from './CubeFaceNodeSemantics.js';
import { resolveCubeFaceRevealDecision } from './CubeFaceRevealPolicy.js';
import { cubeFaceCardColumnSpan, cubeFaceCardMasonryPriority } from './CubeFacePromptPolicy.js';
/** Resolve the exact cards and menu entries consumed by either Comfy renderer. */
export function resolveCubeFaceCardPresentation(nodes, state, graph) {
    const cards = [];
    const menuEntries = [];
    nodes.forEach((node, index) => {
        if (isHardHiddenCubeFaceNode(node))
            return;
        const id = String(node.id ?? index);
        const persisted = state.cards[id];
        const reveal = resolveCubeFaceRevealDecision(node, persisted);
        const showActivationControl = cubeFaceNodeAllowsActivationControl(node) &&
            (reveal.revealable || inferCubeFaceTransformSignals(node).size > 0);
        const eligible = cubeFaceNodeHasVisibleWidgets(node) || showActivationControl || reveal.revealable;
        if (!eligible)
            return;
        const label = node.title?.trim() || node.type?.trim() || `Node ${String(index + 1)}`;
        if (reveal.revealable) {
            menuEntries.push({ id, label, revealed: reveal.revealed });
        }
        cards.push({
            id,
            node,
            label,
            visible: reveal.visible,
            enabled: reveal.enabled,
            showActivationControl,
            columnSpan: cubeFaceCardColumnSpan(node),
        });
    });
    return { cards: orderCubeFaceMasonryCards(cards, nodes, graph), menuEntries };
}
/** Match Substitute's upstream-first card order, then pin semantic prompts above masonry. */
function orderCubeFaceMasonryCards(cards, nodes, graph) {
    const graphOrder = new Map(orderNodesUpstreamFirst(nodes, graph).map((id, index) => [id, index]));
    return cards
        .map((card, index) => ({
        card,
        index,
        priority: cubeFaceCardMasonryPriority(card.node),
        graphIndex: graphOrder.get(card.id) ?? index,
    }))
        .sort((left, right) => left.priority - right.priority ||
        left.graphIndex - right.graphIndex ||
        left.index - right.index)
        .map(({ card }) => card);
}
/** Return a stable complete-graph order so hidden intermediaries retain dependency chronology. */
function orderNodesUpstreamFirst(nodes, graph) {
    const nodeIds = nodes.map((node, index) => nodeId(node, index));
    const nodesById = new Map(nodes.map((node, index) => [nodeId(node, index), node]));
    const inDegree = new Map(nodeIds.map((id) => [id, 0]));
    const dependents = new Map(nodeIds.map((id) => [id, new Set()]));
    for (const [targetId, node] of nodesById) {
        for (const originId of linkedOriginNodeIds(node, graph)) {
            const sourceId = String(originId);
            if (!nodesById.has(sourceId) || sourceId === targetId)
                continue;
            const downstream = dependents.get(sourceId);
            if (!downstream || downstream.has(targetId))
                continue;
            downstream.add(targetId);
            inDegree.set(targetId, (inDegree.get(targetId) ?? 0) + 1);
        }
    }
    const queue = nodeIds.filter((id) => inDegree.get(id) === 0);
    const ordered = [];
    const orderedIds = new Set();
    while (queue.length > 0) {
        const currentId = queue.shift();
        if (!currentId)
            continue;
        ordered.push(currentId);
        orderedIds.add(currentId);
        for (const dependentId of dependents.get(currentId) ?? []) {
            const remaining = (inDegree.get(dependentId) ?? 0) - 1;
            inDegree.set(dependentId, remaining);
            if (remaining === 0)
                queue.push(dependentId);
        }
    }
    return [...ordered, ...nodeIds.filter((id) => !orderedIds.has(id))];
}
/** Return a stable string identifier even when a host node has not received an id yet. */
function nodeId(node, index) {
    return String(node.id ?? index);
}
/** Return source node identifiers for every resolvable incoming connection on one card. */
function linkedOriginNodeIds(node, graph) {
    const ownerGraph = graph ?? node.graph;
    if (!ownerGraph)
        return [];
    return (node.inputs ?? []).flatMap((input) => inputLinkIds(input).flatMap((linkId) => {
        const link = resolveGraphLink(ownerGraph, linkId);
        const originId = link?.origin_id ?? link?.origin;
        return originId === null || originId === undefined ? [] : [originId];
    }));
}
/** Normalize Comfy's singular and plural input-link shapes into one stable sequence. */
function inputLinkIds(input) {
    return [
        ...(input.links ?? []),
        ...(input.link === null || input.link === undefined ? [] : [input.link]),
    ];
}
/** Resolve one host graph link without coupling the ordering policy to a graph implementation. */
function resolveGraphLink(graph, id) {
    const fromMethod = graph.getLink?.(id);
    if (fromMethod)
        return fromMethod;
    const links = graph.links ?? graph._links;
    if (links instanceof Map)
        return links.get(id) ?? null;
    if (Array.isArray(links))
        return links.find((link) => link.id === id) ?? null;
    return links?.[String(id)] ?? null;
}
