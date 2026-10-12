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
/** Decode authored source labels and full instance paths from one workflow-owned serialized snapshot. */
import { isRecord } from '../../types/common.js';
import { outputSourceId } from '../domain/ExecutionIdentity.js';
import { buildCubeOutputExecutionId } from '../../cube/execution/CubeOutputExecutionIdentity.js';
/** Inventory serialized topology once at a load/submission boundary, never during media or pointer events. */
export function decodeComfyWorkflowSources(workflow) {
    if (!record(workflow) || !Array.isArray(workflow.nodes))
        return Object.freeze([]);
    const definitions = new Map();
    const declared = record(workflow.definitions) && Array.isArray(workflow.definitions.subgraphs)
        ? workflow.definitions.subgraphs
        : [];
    for (const definition of declared)
        if (record(definition) && typeof definition.id === 'string')
            definitions.set(definition.id, definition);
    const sources = [];
    const paths = new Set();
    const append = (path, label, breadcrumb) => {
        if (paths.has(path))
            return;
        paths.add(path);
        sources.push(Object.freeze({
            id: outputSourceId(path),
            executionPath: path,
            label,
            breadcrumb: Object.freeze([...breadcrumb]),
            order: sources.length,
        }));
    };
    const visit = (graph, prefix, breadcrumb, ancestors) => {
        if (prefix.length > 32 || ancestors.has(graph) || !Array.isArray(graph.nodes))
            return;
        const nextAncestors = new Set([...ancestors, graph]);
        const nodes = graph.nodes.filter(record).map((node, index) => ({ node, index }));
        nodes.sort((a, b) => order(a.node, a.index) - order(b.node, b.index));
        for (const { node } of nodes) {
            const id = nodeId(node.id);
            if (!id)
                continue;
            const segments = [...prefix, id];
            const path = segments.join(':');
            const definition = typeof node.type === 'string' ? definitions.get(node.type) : undefined;
            const nested = record(node.subgraph) ? node.subgraph : definition;
            const label = text(node.title) || text(nested?.name) || text(node.type) || 'Output';
            append(path, label, breadcrumb);
            const properties = record(node.properties) ? node.properties : null;
            if (nested && properties?.sugarcubes_kind === 'cube' && Array.isArray(nested.outputs))
                nested.outputs.forEach((output, slot) => {
                    const name = record(output) ? text(output.name) : '';
                    append(buildCubeOutputExecutionId(path, slot), name || `Output ${slot + 1}`, [
                        ...breadcrumb,
                        label,
                    ]);
                });
            if (nested)
                visit(nested, segments, [...breadcrumb, label], nextAncestors);
        }
    };
    visit(workflow, [], [], new Set());
    return Object.freeze(sources);
}
/** Reject collection values that cannot represent a serialized graph or node. */
function record(value) {
    return isRecord(value) && !Array.isArray(value);
}
/** Keep authored topology ordering deterministic when a node has no valid execution ordinal. */
function order(node, fallback) {
    return typeof node.order === 'number' && Number.isSafeInteger(node.order) && node.order >= 0
        ? node.order
        : fallback;
}
/** Normalize literal display text without admitting unbounded host strings. */
function text(value) {
    return typeof value === 'string' ? value.trim().slice(0, 4096) : '';
}
/** Accept one local serialized node ID without confusing it with a complete instance path. */
function nodeId(value) {
    if (typeof value === 'number' && Number.isSafeInteger(value) && value >= 0)
        return String(value);
    return typeof value === 'string' &&
        value.length > 0 &&
        value.length <= 4096 &&
        !value.includes(':')
        ? value
        : null;
}
