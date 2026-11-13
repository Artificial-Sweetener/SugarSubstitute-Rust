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
/** Persist validated portable Cube content beside its native Comfy definition. */
import { isRecord } from '../types/common.js';
/** Parse the canonical portable Cube document returned by the backend. */
export function parseCubeDefinitionDocument(value) {
    if (!isRecord(value)) {
        throw new TypeError('Cube save response definition.document must be an object.');
    }
    return {
        ...value,
        cube_id: requireString(value.cube_id, 'document.cube_id'),
        version: requireString(value.version, 'document.version'),
        implementation: requireRecord(value.implementation, 'implementation'),
        surface: requireRecord(value.surface, 'surface'),
        flavors: requireRecord(value.flavors, 'flavors'),
    };
}
/** Validate and copy one canonical Cube document into graph-owned persistence. */
export function writeCubeDefinitionDocument(subgraph, document, identity) {
    if (document === undefined)
        return;
    const canonicalDocument = parseCubeDefinitionDocument(document);
    const cubeId = canonicalDocument.cube_id;
    const version = canonicalDocument.version;
    if (cubeId !== identity.cubeId || version !== identity.cubeVersion) {
        throw new TypeError('Cube document identity disagrees with its native definition.');
    }
    subgraph.extra = {
        ...(isRecord(subgraph.extra) ? subgraph.extra : {}),
        sugarcubes_document: cloneRecord(canonicalDocument),
    };
}
/** Require one canonical portable document section. */
function requireRecord(value, section) {
    if (!isRecord(value)) {
        throw new TypeError(`Cube ${section} document section must be an object.`);
    }
    return value;
}
/** Require one non-empty document identity value. */
function requireString(value, path) {
    const normalized = typeof value === 'string' ? value.trim() : '';
    if (!normalized)
        throw new TypeError(`Cube ${path} must be a non-empty string.`);
    return normalized;
}
/** Detach JSON-domain content from the import response. */
function cloneRecord(value) {
    const parsed = JSON.parse(JSON.stringify(value));
    return isRecord(parsed) ? parsed : {};
}
