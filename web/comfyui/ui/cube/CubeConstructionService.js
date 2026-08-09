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
/** Construct detached native Cube nodes from prepared imports. */
import { isRecord } from '../types/common.js';
import { writeCubeDefinitionIdentity } from './node/CubeDefinitionIdentityWriter.js';
const MINIMUM_CUBE_WIDTH = 240;
const MINIMUM_CUBE_HEIGHT = 160;
/** Own graph assembly, instance identity, metadata, and detached node construction. */
export class CubeConstructionService {
    #graphBuilder;
    #nodeFactory;
    #definitions;
    #resolveInitialSize;
    #createInstanceId;
    /** Bind focused native construction collaborators and rollback ownership. */
    constructor(options) {
        this.#graphBuilder = options.graphBuilder;
        this.#nodeFactory = options.nodeFactory;
        this.#definitions = options.definitions;
        this.#resolveInitialSize = options.resolveInitialSize;
        this.#createInstanceId = options.createInstanceId;
    }
    /** Build one fresh definition and return its detached configured native node. */
    construct(payload, options = {}) {
        const title = resolveTitle(payload, options.instanceAlias);
        const built = this.#graphBuilder.build(payload, `Cube: ${title}`);
        const identity = readPayloadIdentity(payload, options.instanceId ?? this.#createInstanceId(), options.instanceAlias, options.surface, options.revisionRef);
        return this.constructBuilt({
            built,
            title,
            identity,
            geometry: {
                position: readPayloadPosition(payload, options.position),
                size: options.size ??
                    this.#resolveInitialSize({
                        surface: readSurfaceState(identity.metadata),
                        hasInputs: built.subgraph.inputs.length > 0,
                    }),
            },
        });
    }
    /** Configure one prebuilt definition and return a detached real Cube node. */
    constructBuilt(request) {
        const metadata = buildInstanceMetadata(request.identity);
        writeCubeDefinitionIdentity(request.built.subgraph, 'cube', metadata);
        try {
            const node = this.#nodeFactory.create({
                instanceId: requireInstanceId(request.identity.instanceId),
                subgraph: request.built.subgraph,
                title: request.title,
                position: request.geometry.position,
                size: normalizedSize(request.geometry.size),
                identity: metadata,
                surface: readSurfaceState(request.identity.metadata),
            });
            return {
                node,
                subgraph: request.built.subgraph,
                warnings: request.built.warnings,
                internalNodeCount: request.built.nodesBySymbol.size,
            };
        }
        catch (error) {
            this.#definitions.discard(request.built.subgraph);
            throw error;
        }
    }
    /** Discard a detached construction that will not enter the root graph. */
    discard(constructed) {
        this.#definitions.discard(constructed.subgraph);
    }
}
/** Parse stable instance identity at the prepared-import boundary. */
function readPayloadIdentity(payload, instanceId, instanceAlias, surface, revisionRef) {
    const cube = isRecord(payload.cube) ? payload.cube : {};
    const cubeMetadata = isRecord(cube.metadata) ? cube.metadata : {};
    return {
        cubeId: readString(cube.cube_id),
        cubeVersion: readString(cube.version),
        instanceId: requireInstanceId(instanceId),
        defaultAlias: readString(cube.default_alias),
        instanceAlias: readString(instanceAlias) || readString(cube.default_alias),
        metadata: cloneRecord({
            ...cube,
            ...cubeMetadata,
            ...(surface ? { surface_state: surface } : {}),
            ...(revisionRef ? { cube_revision_ref: revisionRef } : {}),
        }),
    };
}
/** Build persisted metadata from one validated Cube identity. */
function buildInstanceMetadata(identity) {
    return {
        ...identity.metadata,
        schema: 1,
        kind: 'cube',
        cube_id: identity.cubeId,
        cube_version: identity.cubeVersion,
        instance_id: requireInstanceId(identity.instanceId),
        default_alias: identity.defaultAlias,
        instance_alias: identity.instanceAlias,
    };
}
/** Read the optional persisted Cube face state. */
function readSurfaceState(metadata) {
    return isRecord(metadata.surface_state) ? cloneRecord(metadata.surface_state) : {};
}
/** Clone JSON-safe metadata before assigning instance ownership. */
function cloneRecord(value) {
    const parsed = JSON.parse(JSON.stringify(value));
    return isRecord(parsed) ? parsed : {};
}
/** Resolve one visible instance title. */
function resolveTitle(payload, instanceAlias) {
    const cube = isRecord(payload.cube) ? payload.cube : {};
    return (readString(instanceAlias) ||
        readString(cube.default_alias) ||
        readString(payload.default_alias) ||
        'SugarCube');
}
/** Read a finite fresh-placement origin without inheriting authored model-space size. */
function readPayloadPosition(payload, explicitPosition) {
    return readPair(explicitPosition ?? payload.layout?.origin, [0, 0]);
}
/** Enforce the minimum finite Cube frame size. */
function normalizedSize(size) {
    return [
        Math.max(MINIMUM_CUBE_WIDTH, Number(size[0]) || 0),
        Math.max(MINIMUM_CUBE_HEIGHT, Number(size[1]) || 0),
    ];
}
/** Read one finite position pair. */
function readPair(value, fallback) {
    if (!Array.isArray(value))
        return fallback;
    const x = Number(value[0]);
    const y = Number(value[1]);
    return [Number.isFinite(x) ? x : fallback[0], Number.isFinite(y) ? y : fallback[1]];
}
/** Require a stable extension-owned instance identity. */
function requireInstanceId(value) {
    const normalized = value.trim();
    if (!normalized)
        throw new Error('Cube instance identity is required.');
    return normalized;
}
/** Read a trimmed metadata string. */
function readString(value) {
    return typeof value === 'string' ? value.trim() : '';
}
