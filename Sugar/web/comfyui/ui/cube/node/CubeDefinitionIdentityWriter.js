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
/** Own Cube identity and product description stored on native definitions. */
import { isRecord } from '../../types/common.js';
/** Write the complete definition-level identity consumed by Comfy serialization. */
export function writeCubeDefinitionIdentity(subgraph, kind, metadata) {
    subgraph.extra = {
        ...(isRecord(subgraph.extra) ? subgraph.extra : {}),
        sugarcubes_kind: kind,
        sugarcubes_cube: cloneRecord(metadata),
    };
    subgraph.description = resolveCubeDefinitionDescription(metadata);
}
/** Derive the host tooltip from authoritative Cube metadata. */
export function resolveCubeDefinitionDescription(metadata) {
    const description = readString(metadata.description);
    if (description)
        return description;
    const alias = readString(metadata.default_alias) || readString(metadata.instance_alias);
    return `SugarCube: ${alias || 'Untitled Cube'}`;
}
/** Clone JSON-domain metadata before it crosses into graph-owned persistence. */
function cloneRecord(value) {
    const parsed = JSON.parse(JSON.stringify(value));
    return isRecord(parsed) ? parsed : {};
}
/** Read one non-empty metadata string. */
function readString(value) {
    return typeof value === 'string' ? value.trim() : '';
}
