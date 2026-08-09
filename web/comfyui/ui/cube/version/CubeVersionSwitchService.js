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
/** Orchestrate exact Cube version loading, state transfer, and atomic replacement. */
import { normalizeCubeVersion } from '../../core/CubeDefinitionKey.js';
import { requireCubeIdentity } from '../node/ComfyCubeNodeFactory.js';
import { readInstanceId } from '../node/CubeNodeCatalog.js';
import { buildCubeVersionBoundaryMap } from './CubeVersionBoundaryMap.js';
import { transferCubeVersionState } from './CubeVersionStateTransfer.js';
/** Replace one persisted Cube instance with a selected available semantic version. */
export class CubeVersionSwitchService {
    #availability;
    #repository;
    #construction;
    #replacement;
    #definitions;
    /** Bind focused collaborators for the version-switch use case. */
    constructor(options) {
        this.#availability = options.availability;
        this.#repository = options.repository;
        this.#construction = options.construction;
        this.#replacement = options.replacement;
        this.#definitions = options.definitions;
    }
    /** Switch the active instance or return it unchanged when it already uses the target version. */
    async switch(node, targetVersion) {
        if (node.properties.sugarcubes_kind === 'cube_draft') {
            throw new Error('Draft Cubes do not have selectable versions.');
        }
        const identity = requireCubeIdentity(node);
        const cubeId = requireString(identity.cube_id, 'Cube identity');
        const currentVersion = normalizeCubeVersion(identity.cube_version);
        const normalizedTarget = normalizeCubeVersion(targetVersion);
        if (!normalizedTarget)
            throw new Error('Cube version is required.');
        if (normalizedTarget === currentVersion)
            return node;
        const options = await this.#availability.list(cubeId, currentVersion);
        const sourceOption = options.find((option) => option.value === currentVersion);
        const targetOption = options.find((option) => option.value === normalizedTarget);
        if (!sourceOption)
            throw new Error(`Current Cube version v${currentVersion} is unavailable.`);
        if (!targetOption)
            throw new Error(`Cube version v${normalizedTarget} is unavailable.`);
        const [sourcePayload, targetPayload] = await Promise.all([
            this.#repository.loadArtifact(cubeId, sourceOption),
            this.#repository.loadArtifact(cubeId, targetOption),
        ]);
        const staged = this.#definitions.stage(targetPayload);
        let constructed = null;
        try {
            constructed = this.#construction.construct(staged.payload, {
                instanceId: readInstanceId(node),
                instanceAlias: readAlias(identity, node),
                position: [Number(node.pos[0]) || 0, Number(node.pos[1]) || 0],
                revisionRef: targetOption.revisionRef,
                size: [Number(node.size[0]) || 0, Number(node.size[1]) || 0],
            });
            requireCompleteConstruction(constructed);
            transferCubeVersionState(node, constructed.node);
            this.#replacement.replace({
                source: node,
                target: constructed.node,
                sourceBoundaries: buildCubeVersionBoundaryMap(sourcePayload, node),
                targetBoundaries: buildCubeVersionBoundaryMap(targetPayload, constructed.node),
            });
            return constructed.node;
        }
        catch (error) {
            if (constructed)
                this.#construction.discard(constructed);
            this.#definitions.discard(staged);
            throw error;
        }
    }
}
/** Reject incomplete definitions before they can replace a runnable instance. */
function requireCompleteConstruction(constructed) {
    if (constructed.warnings.length > 0) {
        throw new Error(`Cube version could not be constructed: ${constructed.warnings.join(' ')}`);
    }
}
/** Preserve the user's instance alias independently of definition metadata changes. */
function readAlias(identity, node) {
    const alias = typeof identity.instance_alias === 'string' ? identity.instance_alias.trim() : '';
    return alias || node.title?.trim() || 'SugarCube';
}
/** Require one normalized string owned by persisted Cube identity. */
function requireString(value, label) {
    const normalized = typeof value === 'string' ? value.trim() : '';
    if (!normalized)
        throw new Error(`${label} is required.`);
    return normalized;
}
