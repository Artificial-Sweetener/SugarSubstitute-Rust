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
/** Migrate retired extension-owned container persistence into native Cube nodes. */
import { isRecord } from '../../types/common.js';
const STORAGE_KEY = 'sugarcubes_containers';
const STORAGE_SCHEMA = 1;
/** Own the one-time compatibility boundary for non-node Cube workflows. */
export class LegacyCubeContainerMigrationAdapter {
    #graph;
    #factory;
    #nodes;
    #logger;
    #setDirtyCanvas;
    #previousConfigure;
    #previousSerialize;
    #configureHook;
    #serializeHook;
    /** Install migration after Comfy and Cube-node lifecycle configuration. */
    constructor(options) {
        this.#graph = options.graph;
        this.#factory = options.factory;
        this.#nodes = options.nodes;
        this.#logger = options.logger;
        this.#setDirtyCanvas = options.setDirtyCanvas ?? (() => undefined);
        this.#previousConfigure = options.graph.onConfigure ?? null;
        this.#previousSerialize = options.graph.onSerialize ?? null;
        this.#configureHook = (data) => {
            this.#previousConfigure?.call(this.#graph, data);
            this.#restore(data);
        };
        this.#serializeHook = (data) => {
            this.#previousSerialize?.call(this.#graph, data);
            removeLegacyEnvelope(data);
        };
        options.graph.onConfigure = this.#configureHook;
        options.graph.onSerialize = this.#serializeHook;
    }
    /** Restore hooks without retaining an internal compatibility path. */
    dispose() {
        if (this.#graph.onConfigure === this.#configureHook) {
            this.#graph.onConfigure = this.#previousConfigure;
        }
        if (this.#graph.onSerialize === this.#serializeHook) {
            this.#graph.onSerialize = this.#previousSerialize;
        }
    }
    /** Convert one retired envelope and remove it from the configured workflow. */
    #restore(data) {
        const extra = isRecord(data.extra) ? data.extra : null;
        if (!extra || extra[STORAGE_KEY] === undefined)
            return;
        let envelope;
        try {
            envelope = readEnvelope(extra[STORAGE_KEY]);
        }
        catch (error) {
            this.#logger.error('SugarCubes could not read legacy Cube-container persistence.', {
                reason: readErrorMessage(error),
                error,
            });
            throw error;
        }
        const migrated = new Map();
        for (const snapshot of envelope.items) {
            const existing = this.#nodes.get(snapshot.id);
            if (existing) {
                migrated.set(snapshot.id, existing);
                continue;
            }
            const subgraph = this.#graph.subgraphs.get(snapshot.definitionId);
            if (!subgraph) {
                this.#logger.warn(`SugarCubes skipped legacy Cube '${snapshot.id}' because definition ` +
                    `'${snapshot.definitionId}' is missing.`);
                continue;
            }
            try {
                const configuration = {
                    subgraph,
                    instanceId: snapshot.id,
                    title: snapshot.title,
                    position: snapshot.position,
                    size: snapshot.size,
                    identity: snapshot.identity,
                    surface: snapshot.surface,
                };
                migrated.set(snapshot.id, this.#factory.create(configuration));
            }
            catch (error) {
                this.#logger.error(`SugarCubes failed to migrate legacy Cube '${snapshot.id}'.`, {
                    reason: readErrorMessage(error),
                    error,
                });
            }
        }
        for (const link of envelope.links)
            this.#restoreLink(link, migrated);
        Reflect.deleteProperty(extra, STORAGE_KEY);
        if (migrated.size > 0)
            this.#setDirtyCanvas(true, true);
    }
    /** Restore one retired manual boundary link through native graph slots. */
    #restoreLink(link, cubes) {
        const origin = this.#resolveEndpoint(link.origin, cubes, 'output');
        const target = this.#resolveEndpoint(link.target, cubes, 'input');
        if (!origin || !target) {
            this.#logger.warn(`SugarCubes could not restore legacy Cube link '${link.id}' of type '${link.type}'.`);
            return;
        }
        try {
            origin.node.connect(origin.slot, target.node, target.slot);
        }
        catch (error) {
            this.#logger.error(`SugarCubes failed to restore legacy Cube link '${link.id}'.`, {
                reason: readErrorMessage(error),
                error,
            });
        }
    }
    /** Resolve one retired endpoint to its native node and slot. */
    #resolveEndpoint(endpoint, cubes, direction) {
        const value = endpoint.kind === 'cube'
            ? cubes.get(endpoint.containerId)
            : this.#findRootNode(endpoint.nodeId);
        if (!isNativeLinkNode(value))
            return null;
        const slots = direction === 'input' ? value.inputs : value.outputs;
        return slots[endpoint.slot] === undefined ? null : { node: value, slot: endpoint.slot };
    }
    /** Resolve stringified legacy numeric IDs without assuming the host key type. */
    #findRootNode(id) {
        const direct = this.#graph.getNodeById(id);
        if (direct !== null && direct !== undefined)
            return direct;
        if (!/^-?\d+$/.test(id))
            return null;
        return this.#graph.getNodeById(Number(id));
    }
}
/** Parse the exact retired persistence schema at its compatibility boundary. */
function readEnvelope(value) {
    if (!isRecord(value) || value.schema !== STORAGE_SCHEMA || !Array.isArray(value.items)) {
        throw new TypeError('Unsupported SugarCubes container persistence schema.');
    }
    return {
        items: value.items.map(readSnapshot),
        links: Array.isArray(value.links) ? value.links.map(readBoundaryLink) : [],
    };
}
/** Validate one retired container snapshot. */
function readSnapshot(value) {
    if (!isRecord(value))
        throw new TypeError('Cube container entry must be an object.');
    return {
        id: requireString(value.id, 'Cube container id'),
        definitionId: requireString(value.definition_id, 'Cube definition id'),
        title: requireString(value.title, 'Cube title'),
        position: readPair(value.pos, 'Cube position'),
        size: readPair(value.size, 'Cube size'),
        identity: cloneRecord(value.identity),
        surface: cloneRecord(value.surface),
    };
}
/** Validate one retired boundary link. */
function readBoundaryLink(value) {
    if (!isRecord(value))
        throw new TypeError('Cube boundary link must be an object.');
    return {
        id: requireString(value.id, 'Cube boundary link id'),
        type: readString(value.type) || '*',
        origin: readEndpoint(value.origin),
        target: readEndpoint(value.target),
    };
}
/** Validate one retired root-node or Cube endpoint. */
function readEndpoint(value) {
    if (!isRecord(value) || !Number.isInteger(value.slot) || Number(value.slot) < 0) {
        throw new TypeError('Cube boundary endpoint is invalid.');
    }
    const slot = Number(value.slot);
    if (value.kind === 'cube') {
        return {
            kind: 'cube',
            containerId: requireString(value.containerId, 'Cube endpoint container id'),
            slot,
        };
    }
    if (value.kind === 'node') {
        return {
            kind: 'node',
            nodeId: requireString(value.nodeId, 'Node endpoint id'),
            slot,
        };
    }
    throw new TypeError('Cube boundary endpoint kind is invalid.');
}
/** Remove the retired envelope after all current serialization owners run. */
function removeLegacyEnvelope(data) {
    if (isRecord(data.extra))
        Reflect.deleteProperty(data.extra, STORAGE_KEY);
}
/** Require one non-empty legacy string. */
function requireString(value, label) {
    const normalized = readString(value);
    if (!normalized)
        throw new TypeError(`${label} is required.`);
    return normalized;
}
/** Read one exact finite legacy coordinate pair. */
function readPair(value, label) {
    if (!Array.isArray(value) ||
        value.length < 2 ||
        !Number.isFinite(value[0]) ||
        !Number.isFinite(value[1])) {
        throw new TypeError(`${label} must contain two finite numbers.`);
    }
    return [Number(value[0]), Number(value[1])];
}
/** Read one trimmed external string. */
function readString(value) {
    return typeof value === 'string' ? value.trim() : '';
}
/** Clone JSON-safe retired metadata before assigning native node ownership. */
function cloneRecord(value) {
    if (!isRecord(value))
        return {};
    const parsed = JSON.parse(JSON.stringify(value));
    return isRecord(parsed) ? parsed : {};
}
/** Narrow a graph value to the native link surface used by migration. */
function isNativeLinkNode(value) {
    return (isRecord(value) &&
        Array.isArray(value.inputs) &&
        Array.isArray(value.outputs) &&
        typeof value.connect === 'function');
}
/** Preserve useful failure context at the dynamic host boundary. */
function readErrorMessage(error) {
    return error instanceof Error ? error.message : String(error);
}
