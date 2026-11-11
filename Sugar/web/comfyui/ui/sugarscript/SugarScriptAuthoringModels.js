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
/** Validate SugarScript authoring responses before graph mutation. */
import { readImportPayload } from '../import/PlacementPayload.js';
import { isRecord } from '../types/common.js';
/** Parse the complete untrusted endpoint response or fail before graph mutation. */
export function readSugarScriptAuthoringResponse(value) {
    if (!isRecord(value))
        throw new TypeError('SugarScript response must be an object.');
    const diagnostics = readDiagnostics(value.diagnostics);
    const valid = value.valid === true;
    const plan = value.plan == null ? null : readPlan(value.plan);
    if (valid !== Boolean(plan)) {
        throw new TypeError('SugarScript response validity disagrees with its workflow plan.');
    }
    return { valid, diagnostics, plan };
}
/** Read one complete plan and enforce referential integrity. */
function readPlan(value) {
    if (!isRecord(value))
        throw new TypeError('SugarScript workflow plan must be an object.');
    const semanticHash = requireText(value.semantic_hash, 'plan.semantic_hash');
    if (!/^[0-9a-f]{64}$/.test(semanticHash)) {
        throw new TypeError('SugarScript workflow semantic hash is invalid.');
    }
    if (!Array.isArray(value.instances) || !Array.isArray(value.connections)) {
        throw new TypeError('SugarScript workflow plan collections are invalid.');
    }
    const instances = value.instances.map(readInstance);
    const instanceIds = new Set(instances.map((instance) => instance.instanceId));
    if (instanceIds.size !== instances.length) {
        throw new TypeError('SugarScript workflow plan contains duplicate instance ids.');
    }
    const connections = value.connections.map(readConnection);
    for (const connection of connections) {
        if (!instanceIds.has(connection.sourceInstanceId) ||
            !instanceIds.has(connection.targetInstanceId)) {
            throw new TypeError('SugarScript workflow connection references an unknown instance.');
        }
    }
    return { semanticHash, instances, connections };
}
/** Read one native Cube instance and its existing importer payload. */
function readInstance(value) {
    if (!isRecord(value))
        throw new TypeError('SugarScript instance must be an object.');
    const payload = readImportPayload(value.payload);
    if (!payload?.cube)
        throw new TypeError('SugarScript instance has no valid Cube payload.');
    return {
        instanceId: requireText(value.instance_id, 'instance.instance_id'),
        alias: requireText(value.alias, 'instance.alias'),
        bypassed: value.bypassed === true,
        payload,
    };
}
/** Read one connection between public Cube boundaries. */
function readConnection(value) {
    if (!isRecord(value))
        throw new TypeError('SugarScript connection must be an object.');
    return {
        sourceInstanceId: requireText(value.source_instance_id, 'connection.source_instance_id'),
        sourceBinding: requireText(value.source_binding, 'connection.source_binding'),
        targetInstanceId: requireText(value.target_instance_id, 'connection.target_instance_id'),
        targetBinding: requireText(value.target_binding, 'connection.target_binding'),
    };
}
/** Read stable located diagnostics from either success or failure responses. */
function readDiagnostics(value) {
    if (!Array.isArray(value))
        throw new TypeError('SugarScript diagnostics must be an array.');
    return value.map((item) => {
        if (!isRecord(item) || !isRecord(item.span)) {
            throw new TypeError('SugarScript diagnostic is invalid.');
        }
        const severity = item.severity;
        if (severity !== 'error' && severity !== 'warning') {
            throw new TypeError('SugarScript diagnostic severity is invalid.');
        }
        return {
            code: requireText(item.code, 'diagnostic.code'),
            severity,
            message: requireText(item.message, 'diagnostic.message'),
            span: {
                start: readPosition(item.span.start),
                end: readPosition(item.span.end),
            },
        };
    });
}
/** Read one finite non-negative source position. */
function readPosition(value) {
    if (!isRecord(value))
        throw new TypeError('SugarScript source position is invalid.');
    const { offset, line, column } = value;
    if (typeof offset !== 'number' || typeof line !== 'number' || typeof column !== 'number') {
        throw new TypeError('SugarScript source position is invalid.');
    }
    if (![offset, line, column].every(Number.isInteger) || offset < 0 || line < 1 || column < 1) {
        throw new TypeError('SugarScript source position is invalid.');
    }
    return { offset, line, column };
}
/** Require one non-empty textual protocol field. */
function requireText(value, field) {
    const text = typeof value === 'string' ? value.trim() : '';
    if (!text)
        throw new TypeError(`SugarScript ${field} is required.`);
    return text;
}
