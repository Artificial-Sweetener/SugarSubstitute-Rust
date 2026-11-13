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
/** Defend Comfy's shared graph and clipboard mutation seams against nested Cube wrappers. */
import { isRecord } from '../../types/common.js';
import { isCubePlacementCandidate } from '../node/ComfyCubeNodeFactory.js';
import { readCubeClipboardViolation } from './CubeClipboardPlacementPreflight.js';
const ADD_MARKER = Symbol.for('SugarCubes.ComfyCubeHostMutationGuard.add');
const DESERIALIZE_MARKER = Symbol.for('SugarCubes.ComfyCubeHostMutationGuard.deserialize');
/** Guard all native add routes while preserving full-workflow hydration for recovery. */
export class ComfyCubeHostMutationGuard {
    #rootGraph;
    #canvas;
    #policy;
    #reportError;
    #owner = {};
    #addOwner = null;
    #installedAdd = null;
    #previousAdd = null;
    #installedDeserialize = null;
    #previousDeserialize = null;
    #addState = null;
    /** Install graph-wide guards before Comfy begins configuring the workflow. */
    constructor(options) {
        this.#rootGraph = options.rootGraph;
        this.#canvas = options.canvas;
        this.#policy = options.policy;
        this.#reportError = options.reportError;
        this.#installAddGuard();
        this.#installDeserializeGuard();
    }
    /** End the narrow hydration exemption after Comfy finishes loading workflow data. */
    completeHydration() {
        if (this.#addState?.owner === this.#owner)
            this.#addState.hydrating = false;
    }
    /** Restore only seams still owned by this runtime instance. */
    dispose() {
        if (this.#addOwner &&
            this.#installedAdd &&
            this.#previousAdd &&
            this.#addOwner.add === this.#installedAdd &&
            readAddState(Reflect.get(this.#installedAdd, ADD_MARKER))?.owner === this.#owner) {
            this.#addOwner.add = this.#previousAdd;
        }
        if (this.#installedDeserialize &&
            this.#previousDeserialize &&
            this.#canvas._deserializeItems === this.#installedDeserialize &&
            readDeserializeState(Reflect.get(this.#installedDeserialize, DESERIALIZE_MARKER))?.owner ===
                this.#owner) {
            this.#canvas._deserializeItems = this.#previousDeserialize;
        }
        this.#addOwner = null;
        this.#installedAdd = null;
        this.#previousAdd = null;
        this.#installedDeserialize = null;
        this.#previousDeserialize = null;
        this.#addState = null;
    }
    /** Wrap the shared LGraph.add implementation and scope it to this workflow root. */
    #installAddGuard() {
        const owner = findMethodOwner(this.#rootGraph, 'add');
        if (!owner || typeof owner.add !== 'function') {
            throw new TypeError('Comfy LGraph.add is unavailable.');
        }
        const current = owner.add;
        const existing = readAddState(Reflect.get(current, ADD_MARKER));
        if (existing) {
            existing.owner = this.#owner;
            existing.rootGraph = this.#rootGraph;
            existing.policy = this.#policy;
            existing.hydrating = true;
            existing.reportError = this.#reportError;
            this.#addOwner = owner;
            this.#installedAdd = current;
            this.#previousAdd = existing.previous;
            this.#addState = existing;
            return;
        }
        const state = {
            namespace: 'SugarCubes.RootOnlyAdd',
            owner: this.#owner,
            rootGraph: this.#rootGraph,
            policy: this.#policy,
            hydrating: true,
            previous: current,
            reportError: this.#reportError,
        };
        const wrapper = function (node, ...args) {
            if (belongsToRoot(this, state.rootGraph) &&
                !state.hydrating &&
                isCubePlacementCandidate(node) &&
                !state.policy.allows(this)) {
                const error = createPlacementError(state.policy, this);
                state.reportError('SugarCube placement unavailable', error.message);
                throw error;
            }
            return Reflect.apply(state.previous, this, [node, ...args]);
        };
        Reflect.set(wrapper, ADD_MARKER, state);
        owner.add = wrapper;
        this.#addOwner = owner;
        this.#installedAdd = wrapper;
        this.#previousAdd = current;
        this.#addState = state;
    }
    /** Preflight clone and paste before Comfy registers copied Subgraph definitions. */
    #installDeserializeGuard() {
        const current = this.#canvas._deserializeItems;
        if (typeof current !== 'function') {
            throw new TypeError('Comfy LGraphCanvas._deserializeItems is unavailable.');
        }
        const existing = readDeserializeState(Reflect.get(current, DESERIALIZE_MARKER));
        if (existing) {
            existing.owner = this.#owner;
            existing.rootGraph = this.#rootGraph;
            existing.policy = this.#policy;
            existing.reportError = this.#reportError;
            this.#installedDeserialize = current;
            this.#previousDeserialize = existing.previous;
            return;
        }
        const state = {
            namespace: 'SugarCubes.RootOnlyDeserialize',
            owner: this.#owner,
            rootGraph: this.#rootGraph,
            policy: this.#policy,
            previous: current,
            reportError: this.#reportError,
        };
        const wrapper = function (parsed, options) {
            const targetGraph = isRecord(this) ? this.graph : null;
            const violation = readCubeClipboardViolation(parsed, state.policy.allows(targetGraph));
            if (violation) {
                const error = createPlacementError(state.policy, targetGraph);
                const detail = `${error.message} ${violation}`;
                state.reportError('SugarCube placement unavailable', detail);
                throw new Error(detail);
            }
            return Reflect.apply(state.previous, this, [parsed, options]);
        };
        Reflect.set(wrapper, DESERIALIZE_MARKER, state);
        this.#canvas._deserializeItems = wrapper;
        this.#installedDeserialize = wrapper;
        this.#previousDeserialize = current;
    }
}
/** Create the policy's typed placement failure without duplicating its wording. */
function createPlacementError(policy, targetGraph) {
    try {
        policy.assertAllowed(targetGraph, 'placed');
    }
    catch (error) {
        return error instanceof Error ? error : new Error(String(error));
    }
    return new Error('SugarCube placement is unavailable.');
}
/** Return whether one graph receiver belongs to the guarded workflow registry. */
function belongsToRoot(value, rootGraph) {
    return value === rootGraph || (isRecord(value) && value.rootGraph === rootGraph);
}
/** Find the object that owns one callable host method so nested graphs share the guard. */
function findMethodOwner(value, method) {
    let candidate = Object.getPrototypeOf(value);
    while (candidate) {
        if (Object.prototype.hasOwnProperty.call(candidate, method))
            return candidate;
        candidate = Object.getPrototypeOf(candidate);
    }
    return Object.prototype.hasOwnProperty.call(value, method) ? value : null;
}
/** Validate a prior add wrapper before hot-reload ownership transfer. */
function readAddState(value) {
    if (!isRecord(value) ||
        value.namespace !== 'SugarCubes.RootOnlyAdd' ||
        typeof value.previous !== 'function' ||
        typeof value.reportError !== 'function' ||
        !isRecord(value.policy) ||
        !isRecord(value.rootGraph) ||
        !isRecord(value.owner)) {
        return null;
    }
    return value;
}
/** Validate a prior clipboard wrapper before hot-reload ownership transfer. */
function readDeserializeState(value) {
    if (!isRecord(value) ||
        value.namespace !== 'SugarCubes.RootOnlyDeserialize' ||
        typeof value.previous !== 'function' ||
        typeof value.reportError !== 'function' ||
        !isRecord(value.policy) ||
        !isRecord(value.rootGraph) ||
        !isRecord(value.owner)) {
        return null;
    }
    return value;
}
