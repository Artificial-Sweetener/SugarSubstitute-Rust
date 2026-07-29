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
/** Write save identity changes to graph-owned Cube nodes. */
import { isRecord } from '../../types/common.js';
import { requireCubeIdentity } from './ComfyCubeNodeFactory.js';
/** Update exactly the native Cube nodes identified by instance id. */
export function updateCubeNodeIdentityForIds(catalog, instanceIds, updates) {
    const targets = new Set(instanceIds.map((value) => value.trim()).filter(Boolean));
    let updated = 0;
    for (const instanceId of targets) {
        const node = catalog.get(instanceId);
        if (!node)
            continue;
        const identity = requireCubeIdentity(node);
        const metadata = applyUpdates(identity, updates);
        replaceRecord(identity, metadata);
        node.subgraph.extra = {
            ...(isRecord(node.subgraph.extra) ? node.subgraph.extra : {}),
            sugarcubes_kind: 'cube',
            sugarcubes_cube: { ...metadata },
        };
        if (updates.defaultAlias)
            node.title = updates.defaultAlias;
        catalog.changed(node);
        updated += 1;
    }
    return updated;
}
/** Apply optional identity values without erasing unrelated metadata. */
function applyUpdates(current, updates) {
    return {
        ...current,
        ...(updates.cubeId ? { cube_id: updates.cubeId } : {}),
        ...(updates.defaultAlias ? { default_alias: updates.defaultAlias } : {}),
        ...(updates.targetModel !== undefined ? { target_model: updates.targetModel } : {}),
        ...(updates.supportedModels ? { supported_models: [...updates.supportedModels] } : {}),
        ...(updates.description !== undefined ? { description: updates.description } : {}),
        ...(updates.cubeVersion ? { cube_version: updates.cubeVersion } : {}),
        ...(updates.cubeRevisionRef ? { cube_revision_ref: updates.cubeRevisionRef } : {}),
        ...(updates.cubeDefinitionKey ? { cube_definition_key: updates.cubeDefinitionKey } : {}),
    };
}
/** Preserve the native-node identity reference while replacing its fields. */
function replaceRecord(target, source) {
    for (const key of Object.keys(target))
        Reflect.deleteProperty(target, key);
    Object.assign(target, source);
}
