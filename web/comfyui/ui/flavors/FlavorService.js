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
/** Compose Flavor event, definition, graph, and command owners. */
import { isRecord } from '../types/common.js';
import { FlavorCommands } from './FlavorCommands.js';
import { FlavorDefinitionLifecycle } from './FlavorDefinitionLifecycle.js';
import { FlavorGraphProjection } from './FlavorGraphProjection.js';
import { FlavorStorage } from './FlavorStorage.js';
import { asFlavorMetadata, errorMessage, } from './FlavorSupport.js';
/** Coordinate the public Flavor API while focused collaborators own behavior. */
export class FlavorService {
    events;
    toast;
    storage;
    graph;
    lifecycle;
    commands;
    unsubscribers = [];
    constructor({ adapter, dialogs, events, storage, toast, api, dirtyManager, cubeBrowser, } = {}) {
        this.events = events || null;
        this.toast = toast || null;
        this.storage = new FlavorStorage({
            ...(storage !== undefined ? { storage } : {}),
            ...(api !== undefined ? { api } : {}),
        });
        this.graph = new FlavorGraphProjection(adapter || null);
        this.lifecycle = new FlavorDefinitionLifecycle({
            dialogs: dialogs || null,
            toast: toast || null,
            dirtyManager: dirtyManager || null,
            storage: this.storage,
            graph: this.graph,
        });
        this.commands = new FlavorCommands({
            dialogs: dialogs || null,
            toast: toast || null,
            api: api || null,
            dirtyManager: dirtyManager || null,
            cubeBrowser: cubeBrowser || null,
            storage: this.storage,
            graph: this.graph,
            lifecycle: this.lifecycle,
        });
    }
    async setup() {
        if (!this.events?.on)
            return;
        this.unsubscribers.push(this.events.on('cube:instances:updated', (payload) => {
            this.refreshGraph(isRecord(payload.graph) ? payload.graph : null);
        }), this.events.on('cube:flavor:change', (payload) => {
            void this.selectFlavor({
                metadata: asFlavorMetadata(payload.metadata),
                flavor: payload.flavor,
            }).catch((error) => {
                this.toast?.push?.('error', 'Flavor selection failed', errorMessage(error, 'Local flavor selection could not be saved.'));
            });
        }), this.events.on('cube:definition:loaded', (payload) => {
            const entry = isRecord(payload.entry) ? payload.entry : undefined;
            const graph = isRecord(payload.graph) ? payload.graph : undefined;
            void this.hydrateFromDefinition({
                ...(typeof payload.cubeId === 'string' ? { cubeId: payload.cubeId } : {}),
                ...(typeof payload.definitionKey === 'string'
                    ? { definitionKey: payload.definitionKey }
                    : {}),
                ...(entry ? { entry } : {}),
                ...(graph ? { graph } : {}),
            }).catch((error) => {
                this.toast?.push?.('warn', 'Cube defaults unavailable', errorMessage(error, 'Cube default state could not be loaded.'));
            });
        }));
    }
    dispose() {
        for (const unsubscribe of this.unsubscribers.splice(0)) {
            try {
                unsubscribe?.();
            }
            catch (_error) {
                /* Listener cleanup is best-effort. */
            }
        }
    }
    buildImportedMetadata(cube) {
        return this.lifecycle.buildImportedMetadata(cube);
    }
    async hydrateFromDefinition(options = {}) {
        return this.lifecycle.hydrate(options);
    }
    refreshGraph(graph) {
        this.lifecycle.refreshGraph(graph);
    }
    refreshGroupMetadata(graph, group, metadata, options = {}) {
        this.lifecycle.refreshGroup(graph, group, metadata, options);
    }
    async reconcileGroupLocalFlavors(graph, group, metadata) {
        await this.lifecycle.reconcileLocalFlavors(graph, group, metadata);
    }
    async promptForLocalFlavorCollisionRenames(options = {}) {
        return this.lifecycle.promptCollisionRenames(options);
    }
    selectionNeedsApplication(metadata, flavor) {
        return this.graph.selectionNeedsApplication(metadata, flavor);
    }
    getGraph() {
        return this.graph.getGraph();
    }
    findGroupByMetadata(graph, metadata) {
        return this.graph.findGroup(graph, metadata);
    }
    buildNodesBySymbol(graph, metadata) {
        return this.graph.buildNodesBySymbol(graph, metadata);
    }
    collectCurrentSurfaceValues(graph, metadata) {
        return this.graph.collectValues(graph, metadata);
    }
    applyFlavorValues(graph, metadata, flavor) {
        this.graph.applyValues(graph, metadata, flavor);
    }
    async selectFlavor({ metadata = {}, flavor = null, } = {}) {
        await this.commands.select(metadata, flavor);
    }
    async saveCurrentFaceValuesAsAuthoredFlavor(metadata) {
        return this.commands.promptAndSaveAuthored(metadata);
    }
    async saveAuthoredFlavor(metadata, options = {}) {
        return this.commands.saveAuthored(metadata, options);
    }
    async loadLocalFlavorState(cubeId) {
        await this.storage.loadCubeState(cubeId);
    }
    mergeAuthoredFlavorValues(existing, next) {
        return this.commands.mergeAuthoredValues(existing, next);
    }
    async saveCurrentFaceValuesAsLocalFlavor(metadata) {
        return this.commands.promptAndSaveLocal(metadata);
    }
    async deleteLocalFlavor(metadata, flavorId) {
        return this.commands.deleteLocal(metadata, flavorId);
    }
    async manageFlavors(metadata) {
        return this.commands.manage(metadata);
    }
}
