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
/** Recover legacy nested-subgraph values from their co-persisted definition clone. */
import { isRecord } from '../../types/common.js';
const CONTROL_AFTER_GENERATE_VALUES = new Set(['fixed', 'increment', 'decrement', 'randomize']);
/** Replace incomplete wrapper arrays with values resolved through saved semantic identities. */
export function reconcileLegacySubgraphInstance(node, cubeMetadata, definitions) {
    const properties = isRecord(node.properties) ? node.properties : {};
    const originalSubgraphId = readString(properties.sugarcubes_original_subgraph_id);
    if (!originalSubgraphId || !Array.isArray(node.widgets_values))
        return;
    if (hasPersistedCubeEdits(cubeMetadata)) {
        throw new Error(`Nested Sugar-DSL node '${nodeLabel(node)}' contains edited positional widget values ` +
            'that cannot be restored safely.');
    }
    const definitionId = readString(node.type);
    const definition = definitions.get(definitionId);
    if (!definition) {
        throw new Error(`Nested Sugar-DSL node '${nodeLabel(node)}' is missing its persisted definition ` +
            `'${definitionId}'.`);
    }
    const declaredOriginalId = readOriginalDefinitionId(definition);
    if (declaredOriginalId && declaredOriginalId !== originalSubgraphId) {
        throw new Error(`Nested Sugar-DSL node '${nodeLabel(node)}' references a mismatched persisted definition.`);
    }
    node.widgets_values = resolveBoundaryWidgetValues(definition);
}
/** Resolve public widget defaults by following saved boundary links to named inner inputs. */
function resolveBoundaryWidgetValues(definition) {
    const nodes = indexRecords(definition.nodes, 'node');
    const links = indexRecords(definition.links, 'link');
    const resolved = [];
    for (const boundary of readRecords(definition.inputs)) {
        const boundaryName = readString(boundary.name);
        const linkIds = Array.isArray(boundary.linkIds) ? boundary.linkIds : [];
        if (!boundaryName || linkIds.length === 0)
            continue;
        const values = linkIds
            .map((linkId) => resolveBoundaryTargetValue(linkId, nodes, links))
            .filter((value) => value.found)
            .map((value) => value.value);
        if (values.length === 0)
            continue;
        if (values.some((value) => !sameJsonValue(value, values[0]))) {
            throw new Error(`Persisted subgraph input '${boundaryName}' has conflicting defaults.`);
        }
        resolved.push(cloneValue(values[0]));
    }
    return resolved;
}
/** Resolve one boundary link to a target widget value stored in the same node snapshot. */
function resolveBoundaryTargetValue(linkId, nodes, links) {
    const link = links.get(idKey(linkId));
    if (!link || idKey(link.origin_id) !== '-10')
        return { found: false };
    const target = nodes.get(idKey(link.target_id));
    const targetSlot = Number(link.target_slot);
    const inputs = target ? readRecords(target.inputs) : [];
    const input = Number.isInteger(targetSlot) && targetSlot >= 0 ? inputs[targetSlot] : undefined;
    const widget = input && isRecord(input.widget) ? input.widget : null;
    const inputName = readString(widget?.name) || readString(input?.name);
    if (!target || !widget || !inputName)
        return { found: false };
    const values = decodeSavedWidgetValues(target);
    return values.has(inputName) ? { found: true, value: values.get(inputName) } : { found: false };
}
/** Decode one saved widget array using only adjacent saved input identities. */
function decodeSavedWidgetValues(node) {
    const names = readRecords(node.inputs)
        .filter((input) => isRecord(input.widget))
        .map((input) => readString(input.widget.name) || readString(input.name))
        .filter(Boolean);
    if (new Set(names).size !== names.length) {
        throw new Error(`Serialized node '${String(node.id ?? '')}' has duplicate widget identities.`);
    }
    const values = Array.isArray(node.widgets_values) ? node.widgets_values : [];
    const candidates = decodeCandidates(names, values, 0, 0, new Map());
    if (candidates.length !== 1) {
        throw new Error(`Serialized node '${String(node.id ?? '')}' has widget values without a unique saved identity.`);
    }
    return candidates[0] ?? new Map();
}
/** Enumerate the bounded companion-widget interpretations and require a unique result. */
function decodeCandidates(names, values, nameIndex, valueIndex, decoded) {
    if (nameIndex === names.length)
        return valueIndex === values.length ? [new Map(decoded)] : [];
    if (valueIndex >= values.length)
        return [];
    const next = new Map(decoded);
    next.set(names[nameIndex] ?? '', values[valueIndex]);
    const candidates = decodeCandidates(names, values, nameIndex + 1, valueIndex + 1, next);
    const companion = values[valueIndex + 1];
    if (typeof companion === 'string' && CONTROL_AFTER_GENERATE_VALUES.has(companion)) {
        candidates.push(...decodeCandidates(names, values, nameIndex + 1, valueIndex + 2, next));
    }
    return candidates;
}
/** Return whether legacy metadata reports edits beyond the cloned definition snapshot. */
function hasPersistedCubeEdits(metadata) {
    return (metadata.implementation_dirty === true ||
        metadata.surface_values_changed === true ||
        metadata.has_saveable_changes === true ||
        metadata.dirty === true);
}
/** Read the original identity asserted by an instance-specific definition clone. */
function readOriginalDefinitionId(definition) {
    const extra = isRecord(definition.extra) ? definition.extra : {};
    const sugar = isRecord(extra.sugar) ? extra.sugar : {};
    return readString(sugar.original_subgraph_id);
}
/** Index persisted records by graph identity while rejecting duplicate IDs. */
function indexRecords(value, label) {
    const index = new Map();
    for (const record of readRecords(value)) {
        const key = idKey(record.id);
        if (!key)
            continue;
        if (index.has(key))
            throw new Error(`Persisted subgraph contains duplicate ${label} id '${key}'.`);
        index.set(key, record);
    }
    return index;
}
/** Return only object records from one untrusted collection. */
function readRecords(value) {
    return Array.isArray(value) ? value.filter(isRecord) : [];
}
/** Normalize a graph identity without accepting missing values. */
function idKey(value) {
    return typeof value === 'string' || typeof value === 'number' ? String(value) : '';
}
/** Read one trimmed persisted string. */
function readString(value) {
    return typeof value === 'string' ? value.trim() : '';
}
/** Produce an actionable persisted-node label for failures. */
function nodeLabel(node) {
    return readString(node.title) || readString(node.type) || String(node.id);
}
/** Compare JSON-safe persisted values without coercion. */
function sameJsonValue(left, right) {
    return JSON.stringify(left) === JSON.stringify(right);
}
/** Clone one JSON-safe widget value before assigning new ownership. */
function cloneValue(value) {
    return value === undefined ? undefined : JSON.parse(JSON.stringify(value));
}
