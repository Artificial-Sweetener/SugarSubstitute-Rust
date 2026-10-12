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
/** Decode submitted execution paths and display metadata without consulting the current graph. */
import { isRecord } from '../../types/common.js';
import { outputSourceId } from '../domain/ExecutionIdentity.js';
/** Read source identities from a compiled prompt; results determine which paths produce visible media. */
export function decodeComfyPromptSources(payload) {
    if (!isRecord(payload) || !isRecord(payload.output))
        return Object.freeze([]);
    const sources = [];
    for (const [executionPath, node] of Object.entries(payload.output)) {
        if (!executionPath || executionPath.length > 4096 || !isRecord(node))
            continue;
        const meta = isRecord(node._meta) ? node._meta : null;
        const title = text(meta?.title) || text(node.class_type) || `Node ${executionPath}`;
        sources.push(Object.freeze({
            id: outputSourceId(executionPath),
            executionPath,
            label: title,
            breadcrumb: Object.freeze(executionPath.split(':').slice(0, -1)),
            order: sources.length,
        }));
    }
    return Object.freeze(sources);
}
/** Bound external labels while preserving literal user-authored text. */
function text(value) {
    return typeof value === 'string' ? value.trim().slice(0, 4096) : '';
}
