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
/** Serialize extracted group-era Cube data as a native Comfy subgraph definition. */
import { isRecord } from '../../types/common.js';
import { reconcileLegacySubgraphInstance } from './LegacySubgraphInstanceReconciler.js';
/** Own conversion from persisted legacy records to Comfy's native subgraph schema. */
export class LegacyCubeDefinitionSerializer {
    #createId;
    /** Bind stable identity generation for graph-owned boundary slots. */
    constructor(createId) {
        this.#createId = createId;
    }
    /** Build one detached marker-free native subgraph definition. */
    serialize(plan, definitionId) {
        const retainedLinkIds = collectRetainedLinkIds(plan);
        const nodes = plan.nodes.map((node) => serializeNode(node, plan.position, retainedLinkIds, plan.metadata, plan.embeddedSubgraphDefinitions));
        const groups = plan.groups.map((group) => serializeGroup(group, plan.position));
        const internalLinks = plan.internalLinks.map(serializeLink);
        const inputLinks = plan.inputs.flatMap((input, inputSlot) => input.targets.map((target) => ({
            id: target.linkId,
            origin_id: -10,
            origin_slot: inputSlot,
            target_id: target.nodeId,
            target_slot: target.slot,
            type: input.type,
        })));
        const outputLinks = plan.outputs.map((output, outputSlot) => ({
            id: output.source.linkId,
            origin_id: output.source.nodeId,
            origin_slot: output.source.slot,
            target_id: -20,
            target_slot: outputSlot,
            type: output.type,
        }));
        const geometry = measureNodes(nodes);
        const inputX = geometry.minX - 125;
        const outputX = geometry.maxX + 50;
        const boundaryHeight = (count) => Math.max(100, 40 + count * 20);
        return {
            id: definitionId,
            name: `Cube: ${plan.title}`,
            version: 1,
            state: {
                lastNodeId: maxNumericId(plan.nodes.map((node) => node.id)),
                lastLinkId: maxNumericId([...retainedLinkIds]),
                lastRerouteId: 0,
                lastGroupId: maxNumericId(plan.groups.map((group) => group.id)),
            },
            revision: 0,
            config: {},
            inputNode: {
                id: -10,
                bounding: [inputX, geometry.minY, 75, boundaryHeight(plan.inputs.length)],
            },
            outputNode: {
                id: -20,
                bounding: [outputX, geometry.minY, 75, boundaryHeight(plan.outputs.length)],
            },
            inputs: plan.inputs.map((input) => ({
                id: this.#createId(),
                name: input.name,
                localized_name: input.name,
                type: input.type,
                linkIds: input.targets.map((target) => target.linkId),
            })),
            outputs: plan.outputs.map((output) => ({
                id: this.#createId(),
                name: output.name,
                localized_name: output.name,
                type: output.type,
                linkIds: [output.source.linkId],
            })),
            widgets: [],
            nodes,
            groups,
            links: [...internalLinks, ...inputLinks, ...outputLinks],
            reroutes: [],
            extra: {},
        };
    }
}
/** Collect links that remain meaningful inside the native Cube graph. */
function collectRetainedLinkIds(plan) {
    const ids = new Set();
    for (const link of plan.internalLinks)
        ids.add(link.id);
    for (const input of plan.inputs) {
        for (const target of input.targets)
            ids.add(target.linkId);
    }
    for (const output of plan.outputs)
        ids.add(output.source.linkId);
    return ids;
}
/** Serialize one real node while retaining only links now owned by the subgraph. */
function serializeNode(source, origin, retainedLinkIds, cubeMetadata, embeddedSubgraphDefinitions) {
    const node = cloneRecord(source);
    reconcileLegacySubgraphInstance(node, cubeMetadata, embeddedSubgraphDefinitions);
    assertSavedWidgetIdentities(node);
    node.pos = shiftPair(node.pos, origin);
    if (Array.isArray(node.inputs)) {
        node.inputs = node.inputs.map((value) => {
            if (!isRecord(value))
                return value;
            const input = { ...value };
            if (!hasId(retainedLinkIds, input.link))
                input.link = null;
            return input;
        });
    }
    if (Array.isArray(node.outputs)) {
        node.outputs = node.outputs.map((value) => {
            if (!isRecord(value))
                return value;
            const output = { ...value };
            const links = Array.isArray(output.links)
                ? output.links.filter((linkId) => hasId(retainedLinkIds, linkId))
                : [];
            output.links = links.length > 0 ? links : null;
            return output;
        });
    }
    return node;
}
/** Reject direct positional arrays that lack identities from their own saved snapshot. */
function assertSavedWidgetIdentities(node) {
    const properties = isRecord(node.properties) ? node.properties : {};
    if (readOriginalSubgraphId(properties) || !Array.isArray(node.widgets_values))
        return;
    if (node.widgets_values.length === 0)
        return;
    const names = Array.isArray(node.inputs)
        ? node.inputs.filter(isRecord).filter((input) => isRecord(input.widget))
        : [];
    if (names.length === 0) {
        throw new Error(`Node '${String(properties.sugarcubes_symbol ?? node.title ?? node.type ?? node.id)}' ` +
            'contains positional widget values without same-snapshot identities.');
    }
}
/** Read the legacy marker that identifies an instance-specific subgraph clone. */
function readOriginalSubgraphId(properties) {
    const value = properties.sugarcubes_original_subgraph_id;
    return typeof value === 'string' ? value.trim() : '';
}
/** Serialize one internal editor group relative to the native Cube origin. */
function serializeGroup(source, origin) {
    const group = cloneRecord(source);
    if (Array.isArray(group.bounding)) {
        const bounds = group.bounding;
        group.bounding = [
            finite(bounds[0]) - origin[0],
            finite(bounds[1]) - origin[1],
            finite(bounds[2]),
            finite(bounds[3]),
        ];
    }
    return group;
}
/** Convert a legacy tuple-derived link to current Comfy object serialization. */
function serializeLink(link) {
    return {
        id: link.id,
        origin_id: link.originNodeId,
        origin_slot: link.originSlot,
        target_id: link.targetNodeId,
        target_slot: link.targetSlot,
        type: link.type,
    };
}
/** Measure relative node geometry for native boundary-node placement. */
function measureNodes(nodes) {
    if (nodes.length === 0)
        return { minX: 150, minY: 100, maxX: 650 };
    let minX = Number.POSITIVE_INFINITY;
    let minY = Number.POSITIVE_INFINITY;
    let maxX = Number.NEGATIVE_INFINITY;
    for (const node of nodes) {
        const pos = readPair(node.pos);
        const size = readPair(node.size, [200, 100]);
        minX = Math.min(minX, pos[0]);
        minY = Math.min(minY, pos[1]);
        maxX = Math.max(maxX, pos[0] + size[0]);
    }
    return { minX, minY, maxX };
}
/** Shift one serialized point while preserving finite coordinates. */
function shiftPair(value, origin) {
    const pair = readPair(value);
    return [pair[0] - origin[0], pair[1] - origin[1]];
}
/** Read one finite numeric pair. */
function readPair(value, fallback = [0, 0]) {
    if (!Array.isArray(value))
        return fallback;
    return [finite(value[0], fallback[0]), finite(value[1], fallback[1])];
}
/** Normalize one finite number. */
function finite(value, fallback = 0) {
    const number = Number(value);
    return Number.isFinite(number) ? number : fallback;
}
/** Find the highest non-negative numeric graph ID. */
function maxNumericId(values) {
    let maximum = 0;
    for (const value of values) {
        const numeric = Number(value);
        if (Number.isInteger(numeric) && numeric >= 0)
            maximum = Math.max(maximum, numeric);
    }
    return maximum;
}
/** Compare graph IDs without depending on whether persistence used strings or numbers. */
function hasId(ids, value) {
    for (const id of ids) {
        if (String(id) === String(value))
            return true;
    }
    return false;
}
/** Clone persisted JSON data before normalizing it for a native graph. */
function cloneRecord(value) {
    const parsed = JSON.parse(JSON.stringify(value));
    return isRecord(parsed) ? parsed : {};
}
