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
/** Route stable Comfy command objects through Cube policy while preserving native behavior. */
import { isRecord } from '../types/common.js';
/** Own command and keybinding routing at Comfy's stable command-object boundary. */
export class ComfyCubeCommandAdapter {
    #document;
    #canvas;
    #contexts;
    #policy;
    #controller;
    #originals = [];
    /** Bind semantic collaborators without importing Comfy's hashed frontend modules. */
    constructor(options) {
        this.#document = options.document;
        this.#canvas = options.canvas;
        this.#contexts = options.contexts;
        this.#policy = options.policy;
        this.#controller = options.controller;
    }
    /** Wrap the existing command objects in place so keybindings retain object identity. */
    install() {
        if (this.#originals.length > 0)
            return;
        const store = requireCommandStore(this.#document);
        for (const binding of this.#bindings()) {
            const command = store.getCommand(binding.commandId);
            if (!command || typeof command.function !== 'function') {
                throw new Error(`Comfy command '${binding.commandId}' is unavailable.`);
            }
            const original = {
                command,
                execute: command.function,
                label: command._label,
                menubarLabel: command._menubarLabel,
            };
            this.#originals.push(original);
            command.function = (metadata) => binding.execute(original.execute, metadata);
            command._label = () => this.#decision(binding.actionId).label;
            command._menubarLabel = () => this.#decision(binding.actionId).label;
        }
    }
    /** Restore the exact native command objects and functions. */
    dispose() {
        for (const original of this.#originals) {
            original.command.function = original.execute;
            original.command._label = original.label;
            original.command._menubarLabel = original.menubarLabel;
        }
        this.#originals.length = 0;
    }
    /** Declare every core command whose meaning changes at a Cube boundary. */
    #bindings() {
        return [
            this.#structuralBinding('Comfy.Graph.ConvertToSubgraph', 'convert'),
            this.#structuralBinding('Comfy.Graph.UnpackSubgraph', 'unpack'),
            {
                commandId: 'Comfy.PublishSubgraph',
                actionId: 'publish',
                execute: (native, metadata) => {
                    const selection = this.#selection();
                    const node = selection.cubeNodes[0];
                    return selection.isSingleCube && node
                        ? this.#controller.saveCube(node)
                        : native(metadata);
                },
            },
            {
                commandId: 'Comfy.Graph.EditSubgraphWidgets',
                actionId: 'configure-interface',
                execute: (native, metadata) => {
                    const selection = this.#selection();
                    if (selection.containsCube || this.#isCubeRoot()) {
                        this.#controller.blocked('configure-interface');
                        return;
                    }
                    return native(metadata);
                },
            },
            {
                commandId: 'Comfy.SaveWorkflow',
                actionId: 'save-workflow',
                execute: (native, metadata) => this.#isCubeRoot()
                    ? void this.#controller.saveActiveEditor(this.#currentGraph())
                    : native(metadata),
            },
            {
                commandId: 'Comfy.Graph.ExitSubgraph',
                actionId: 'exit-container',
                execute: (native, metadata) => native(metadata),
            },
            {
                commandId: 'Comfy.Subgraph.SetDescription',
                actionId: 'set-description',
                execute: (native, metadata) => {
                    if (!this.#isCubeRoot())
                        return native(metadata);
                    const description = metadata?.description;
                    this.#controller.setDescription(description === undefined ? null : String(description));
                },
            },
            {
                commandId: 'Comfy.Subgraph.SetSearchAliases',
                actionId: 'set-search-aliases',
                execute: (native, metadata) => {
                    if (!this.#isCubeRoot())
                        return native(metadata);
                    this.#controller.blocked('search-aliases');
                },
            },
            {
                commandId: 'Comfy.ClearWorkflow',
                actionId: 'clear-container',
                execute: (native, metadata) => this.#isCubeRoot()
                    ? void this.#controller.clearActiveCube(this.#currentGraph())
                    : native(metadata),
            },
        ];
    }
    /** Preserve native operations unless a selected operand is a Cube. */
    #structuralBinding(commandId, actionId) {
        return {
            commandId,
            actionId,
            execute: (native, metadata) => {
                if (!this.#selection().containsCube)
                    return native(metadata);
                this.#controller.blocked(actionId);
            },
        };
    }
    /** Resolve one dynamic policy decision for command labels and execution. */
    #decision(actionId) {
        return this.#policy.decide(actionId, this.#selection(), this.#contexts.resolveEditor(this.#currentGraph()));
    }
    /** Read the current renderer-independent selection classification. */
    #selection() {
        return this.#contexts.resolveSelection(this.#canvas.selectedItems);
    }
    /** Read the graph currently displayed by either Comfy renderer. */
    #currentGraph() {
        return this.#canvas.subgraph ?? this.#canvas.graph ?? null;
    }
    /** Limit editor-command overrides to the root of a Cube definition. */
    #isCubeRoot() {
        return this.#contexts.resolveEditor(this.#currentGraph())?.isCubeRoot === true;
    }
}
/** Resolve the mounted Pinia store by its stable store id and Vue application root. */
function requireCommandStore(documentRef) {
    const appRoot = documentRef.querySelector('#vue-app') ?? documentRef.querySelector('[data-v-app]');
    const vueApp = appRoot ? Reflect.get(appRoot, '__vue_app__') : null;
    const config = isRecord(vueApp) && isRecord(vueApp.config) ? vueApp.config : null;
    const globals = config && isRecord(config.globalProperties) ? config.globalProperties : null;
    const pinia = globals && isRecord(globals.$pinia) ? globals.$pinia : null;
    const stores = pinia ? pinia._s : null;
    const commandStore = stores instanceof Map ? stores.get('command') : null;
    if (!isRecord(commandStore) || typeof commandStore.getCommand !== 'function') {
        throw new Error('Comfy command store compatibility contract is unavailable.');
    }
    return commandStore;
}
