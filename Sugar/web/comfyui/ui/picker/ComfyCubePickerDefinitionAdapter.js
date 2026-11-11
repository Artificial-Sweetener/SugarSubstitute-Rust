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
/** Adapt incremental picker definitions to Comfy's registration and Vue lifecycle. */
import { isRecord } from '../types/common.js';
import { isComfyCubePickerType } from './ComfyCubeNodeDefProjector.js';
/** Own definition contribution, ordering, registration, and stale-type removal. */
export class ComfyCubePickerDefinitionAdapter {
    #registry;
    #app;
    #liteGraph;
    #registeredDefinitions = new Map();
    #hostDefinitions = {};
    #reconciling = null;
    #queuedReconciliation = null;
    /** Bind catalog state to the narrow current Comfy definition capabilities. */
    constructor(options) {
        this.#registry = options.registry;
        this.#app = options.app;
        this.#liteGraph = options.liteGraph;
    }
    /** Contribute placement-ready definitions through Comfy's official startup hook. */
    async contribute(definitions) {
        await this.#registry.refresh();
        const projected = this.#registry.definitions();
        for (const definition of projected)
            definitions[definition.name] = toHostDefinition(definition);
        this.#registeredDefinitions = new Map(projected.map((definition) => [definition.name, JSON.stringify(definition)]));
        this.#captureHostDefinitions(Object.values(definitions));
    }
    /** Ensure cold consumers wait for validation without refreshing a ready catalog. */
    async ensureReady() {
        if (!this.#registry.revision)
            await this.reconcile();
    }
    /** Keep Sugar first among ordinary definitions so Blueprints remain immediately above it. */
    orderForVue(definitions) {
        this.#captureHostDefinitions(definitions);
        for (let index = definitions.length - 1; index >= 0; index -= 1) {
            if (isComfyCubePickerType(definitions[index]?.name))
                definitions.splice(index, 1);
        }
        definitions.unshift(...this.#registry.definitions().map(toHostDefinition));
    }
    /** Serialize registration and queue invalidations that arrive during an active refresh. */
    reconcile() {
        if (this.#reconciling) {
            if (this.#queuedReconciliation)
                return this.#queuedReconciliation;
            const resume = () => {
                this.#queuedReconciliation = null;
                return this.reconcile();
            };
            const queued = this.#reconciling.then(resume, resume);
            this.#queuedReconciliation = queued;
            return queued;
        }
        const operation = this.#reconcile().finally(() => {
            if (this.#reconciling === operation)
                this.#reconciling = null;
        });
        this.#reconciling = operation;
        return operation;
    }
    /** Register only changed Sugar definitions and publish the retained host catalog locally. */
    async #reconcile() {
        const refresh = await this.#registry.refresh({ force: true });
        const updated = refresh.definitions.filter((definition) => this.#registeredDefinitions.get(definition.name) !== JSON.stringify(definition));
        const nextTypes = new Set(refresh.definitions.map(({ name }) => name));
        const removed = [...this.#registeredDefinitions.keys()].filter((type) => !nextTypes.has(type));
        if (!updated.length && !removed.length)
            return;
        const registerNodeDef = this.#app.registerNodeDef;
        if (typeof registerNodeDef !== 'function') {
            throw new Error('Comfy node-definition registration is unavailable.');
        }
        for (const definition of updated) {
            await registerNodeDef.call(this.#app, definition.name, toHostDefinition(definition));
        }
        for (const staleType of removed)
            this.#liteGraph.unregisterNodeType?.(staleType);
        if (typeof this.#app.publishNodeDefs === 'function') {
            await this.#app.publishNodeDefs({
                ...this.#hostDefinitions,
                ...Object.fromEntries(refresh.definitions.map((definition) => [definition.name, toHostDefinition(definition)])),
            });
        }
        this.#registeredDefinitions = new Map(refresh.definitions.map((definition) => [definition.name, JSON.stringify(definition)]));
    }
    /** Retain ordinary host definitions because Comfy's public Vue API replaces its whole store. */
    #captureHostDefinitions(definitions) {
        this.#hostDefinitions = Object.fromEntries(definitions.flatMap((definition) => {
            const name = definition.name;
            return typeof name === 'string' && !isComfyCubePickerType(name) ? [[name, definition]] : [];
        }));
    }
}
/** Copy the authored definition across the untyped Comfy adapter boundary. */
function toHostDefinition(definition) {
    const value = definition;
    if (!isRecord(value))
        throw new TypeError('Cube picker definition is invalid.');
    return value;
}
