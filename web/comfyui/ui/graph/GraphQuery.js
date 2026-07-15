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
/**
 * Own the SugarCubes graph integration layer in `web/comfyui/ui/graph/GraphQuery.js`.
 */
import { isRecord } from '../types/common.js';
/**
 * Get graph nodes.
 */
export function getGraphNodes(graph) {
    if (!isRecord(graph))
        return [];
    if (Array.isArray(graph._nodes))
        return graph._nodes;
    if (Array.isArray(graph.nodes))
        return graph.nodes;
    return [];
}
/**
 * Get graph groups.
 */
export function getGraphGroups(graph) {
    if (!isRecord(graph))
        return [];
    if (Array.isArray(graph._groups))
        return graph._groups;
    if (Array.isArray(graph.groups))
        return graph.groups;
    return [];
}
function normalizeLinkId(value) {
    if (typeof value === 'number' && Number.isFinite(value)) {
        return String(value);
    }
    if (typeof value === 'string') {
        const trimmed = value.trim();
        return trimmed ? trimmed : '';
    }
    if (value == null) {
        return '';
    }
    return String(value);
}
function coerceLinkId(value) {
    const normalized = normalizeLinkId(value);
    if (!normalized) {
        return null;
    }
    const asNumber = Number(normalized);
    if (Number.isFinite(asNumber) && String(asNumber) === normalized) {
        return asNumber;
    }
    return normalized;
}
function collectLinksFromContainer(container) {
    if (!container) {
        return [];
    }
    const links = [];
    const append = (value) => {
        if (isRecord(value)) {
            links.push(value);
        }
    };
    if (Array.isArray(container)) {
        container.forEach(append);
        return links;
    }
    if (container instanceof Map) {
        container.forEach(append);
        return links;
    }
    if (typeof container === 'object') {
        for (const value of Object.values(container)) {
            append(value);
        }
    }
    return links;
}
function buildLinkKey(link) {
    if (!link || typeof link !== 'object') {
        return '';
    }
    const id = normalizeLinkId(link.id);
    if (id) {
        return `id:${id}`;
    }
    const origin = normalizeLinkId(link.origin_id ?? link.origin);
    const originSlot = normalizeLinkId(link.origin_slot);
    const target = normalizeLinkId(link.target_id ?? link.target);
    const targetSlot = normalizeLinkId(link.target_slot);
    if (!origin || !target) {
        return '';
    }
    return `${origin}:${originSlot}->${target}:${targetSlot}`;
}
function dedupeLinks(links) {
    const deduped = [];
    const seen = new Set();
    for (const link of links) {
        const key = buildLinkKey(link);
        if (key && seen.has(key)) {
            continue;
        }
        if (key) {
            seen.add(key);
        }
        deduped.push(link);
    }
    return deduped;
}
function collectLinksFromSlots(graph) {
    const nodes = getGraphNodes(graph);
    if (!nodes.length) {
        return [];
    }
    const inputsByLinkId = new Map();
    for (const node of nodes) {
        if (node?.id == null || !Array.isArray(node.inputs)) {
            continue;
        }
        for (let slotIndex = 0; slotIndex < node.inputs.length; slotIndex += 1) {
            const input = node.inputs[slotIndex];
            const key = normalizeLinkId(input?.link);
            if (!key) {
                continue;
            }
            if (!inputsByLinkId.has(key)) {
                inputsByLinkId.set(key, []);
            }
            inputsByLinkId.get(key)?.push({
                target_id: node.id,
                target_slot: slotIndex,
                type: input?.type ?? null,
            });
        }
    }
    const linksById = new Map();
    for (const node of nodes) {
        if (node?.id == null || !Array.isArray(node.outputs)) {
            continue;
        }
        for (let slotIndex = 0; slotIndex < node.outputs.length; slotIndex += 1) {
            const output = node.outputs[slotIndex];
            const outputLinks = Array.isArray(output?.links)
                ? output.links
                : output?.link != null
                    ? [output.link]
                    : [];
            for (const linkId of outputLinks) {
                const key = normalizeLinkId(linkId);
                if (!key) {
                    continue;
                }
                const existing = linksById.get(key) ?? {
                    id: coerceLinkId(linkId),
                    origin_id: node.id,
                    origin_slot: slotIndex,
                    target_id: null,
                    target_slot: null,
                    type: output?.type ?? null,
                };
                if (existing.origin_id == null) {
                    existing.origin_id = node.id;
                }
                if (existing.origin_slot == null) {
                    existing.origin_slot = slotIndex;
                }
                if (!existing.type && output?.type) {
                    existing.type = output.type;
                }
                const targets = inputsByLinkId.get(key);
                if (Array.isArray(targets) && targets.length) {
                    const [target] = targets;
                    existing.target_id = target?.target_id ?? null;
                    existing.target_slot = target?.target_slot ?? null;
                    if (!existing.type && target?.type) {
                        existing.type = target.type;
                    }
                }
                linksById.set(key, existing);
            }
        }
    }
    for (const [key, targets] of inputsByLinkId.entries()) {
        if (linksById.has(key) || !targets.length) {
            continue;
        }
        const [target] = targets;
        linksById.set(key, {
            id: coerceLinkId(key),
            origin_id: null,
            origin_slot: null,
            target_id: target?.target_id ?? null,
            target_slot: target?.target_slot ?? null,
            type: target?.type ?? null,
        });
    }
    return Array.from(linksById.values());
}
/**
 * Collect graph links.
 */
export function collectGraphLinks(graph) {
    if (!isRecord(graph)) {
        return [];
    }
    const collected = dedupeLinks([
        ...collectLinksFromContainer(graph.links),
        ...collectLinksFromContainer(graph._links),
    ]).filter(Boolean);
    if (collected.length) {
        return collected;
    }
    return dedupeLinks(collectLinksFromSlots(graph)).filter(Boolean);
}
/**
 * Build link index.
 */
export function buildLinkIndex(graph) {
    const outgoing = new Map();
    const incoming = new Map();
    const links = collectGraphLinks(graph);
    for (const link of links) {
        const originId = link.origin_id ?? link.origin;
        const targetId = link.target_id ?? link.target;
        if (originId == null || targetId == null) {
            continue;
        }
        const originKey = String(originId);
        const targetKey = String(targetId);
        const outList = outgoing.get(originKey) ?? [];
        outList.push(link);
        outgoing.set(originKey, outList);
        const inList = incoming.get(targetKey) ?? [];
        inList.push(link);
        incoming.set(targetKey, inList);
    }
    return { outgoing, incoming, links };
}
