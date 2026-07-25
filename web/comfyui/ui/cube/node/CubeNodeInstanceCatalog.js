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
/** Read save identities from graph-owned Cube nodes. */
import { isRecord } from '../../types/common.js';
import { requireCubeIdentity, requireCubeSurface } from './ComfyCubeNodeFactory.js';
import { readInstanceId } from './CubeNodeCatalog.js';
/** Return every persistable Cube node from the live native catalog. */
export function listCubeNodeInstances(nodes) {
    const instances = [];
    for (const node of nodes) {
        const metadata = readNodeMetadata(node);
        const cubeId = readString(metadata.cube_id);
        const definitionId = readString(node.subgraph.id);
        if (!cubeId || !definitionId)
            continue;
        instances.push({
            nodeId: String(node.id),
            definitionId,
            cubeId,
            defaultAlias: readString(metadata.default_alias) || node.title || cubeId,
            instanceId: readInstanceId(node),
            cubeVersion: readString(metadata.cube_version),
            cubeRevisionRef: readString(metadata.cube_revision_ref),
            cubeDefinitionKey: readString(metadata.cube_definition_key),
            targetModel: readString(metadata.target_model),
            supportedModels: readStrings(metadata.supported_models),
            surfaceSize: [Number(node.size[0]), Number(node.size[1])],
            surfaceState: cloneRecord(requireCubeSurface(node)),
        });
    }
    return instances;
}
/** Prefer instance-owned metadata while retaining definition fallback metadata. */
function readNodeMetadata(node) {
    const identity = requireCubeIdentity(node);
    if (Object.keys(identity).length)
        return identity;
    return isRecord(node.subgraph.extra?.sugarcubes_cube) ? node.subgraph.extra.sugarcubes_cube : {};
}
/** Read unique non-empty strings from an untrusted metadata value. */
function readStrings(value) {
    if (!Array.isArray(value))
        return [];
    return Array.from(new Set(value.map(readString).filter(Boolean)));
}
/** Read one trimmed string. */
function readString(value) {
    return typeof value === 'string' ? value.trim() : '';
}
/** Clone JSON-safe face state before crossing into save orchestration. */
function cloneRecord(value) {
    if (!isRecord(value))
        return {};
    const parsed = JSON.parse(JSON.stringify(value));
    return isRecord(parsed) ? parsed : {};
}
