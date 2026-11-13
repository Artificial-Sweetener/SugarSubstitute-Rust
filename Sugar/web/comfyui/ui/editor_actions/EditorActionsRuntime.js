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
/** Compose editor-action collaborators and their renderer lifecycle at the host boundary. */
import { createComfyRendererModeChangeSource, resolveComfyRendererMode, } from '../core/ComfyRendererMode.js';
import { UserPresetLibrary } from './application/UserPresetLibrary.js';
import { ComfyPresetRepository } from './infrastructure/ComfyPresetRepository.js';
import { editorModelScopes } from './infrastructure/EditorModelScopeAdapter.js';
import { EditorContextMenuHost } from './presentation/EditorContextMenuHost.js';
import { canvasDimensionMenuEntries } from './presentation/CanvasDimensionMenu.js';
import { CanvasCancellation } from '../core/embedded/CanvasRuntime.js';
import { readNativeEditorCard } from './infrastructure/NativeEditorCardAdapter.js';
import { isRecord } from '../types/common.js';
/** Keep storage, appearance, node values and menu policy in separate focused owners. */
export class EditorActionsRuntime {
    #options;
    #library;
    #host;
    #release = null;
    #generation = 0;
    #dimensionTasks = new Set();
    /** Prepare explicit collaborators without loading or changing user libraries during construction. */
    constructor(options) {
        this.#options = options;
        this.#library = new UserPresetLibrary(new ComfyPresetRepository(options.fetchApi), () => crypto.randomUUID().replaceAll('-', ''), () => new Date().toISOString());
        this.#host = new EditorContextMenuHost({
            document: options.document,
            library: this.#library,
            reportError: options.reportError,
            skipped: options.skipped,
            modelContributions: options.modelContributions,
            canvasDimensions: (binding) => this.#dimensionEntries(binding),
            readCard: (root) => readNativeEditorCard(root, () => (isRecord(options.app) ? options.app.graph : null)),
            enabled: () => resolveComfyRendererMode(options.app, null, options.document) === 'vue',
            families: (card) => editorModelScopes(card, this.#host.preparedCards(), options.catalog.current()),
        });
    }
    /** Warm library state outside menu opening and subscribe through the existing renderer owner. */
    async setup() {
        const generation = ++this.#generation;
        try {
            await this.#library.prepare();
        }
        catch (error) {
            this.#options.reportError(error);
        }
        if (generation !== this.#generation)
            return;
        this.#host.setup();
        this.#release ??= createComfyRendererModeChangeSource(this.#options.app).subscribe(() => this.#host.refresh());
        this.#options.document.defaultView?.addEventListener('pagehide', this.#pagehide);
    }
    /** Release editor accessories before the host graph and Vue application disappear. */
    dispose() {
        this.#generation += 1;
        for (const cancellation of this.#dimensionTasks)
            cancellation.cancel();
        this.#dimensionTasks.clear();
        this.#release?.();
        this.#release = null;
        this.#host.dispose();
        this.#options.document.defaultView?.removeEventListener('pagehide', this.#pagehide);
    }
    /** Adapt page lifetime without introducing a second graph lifecycle owner. */
    #pagehide = () => this.dispose();
    /** Scope explicit dimension commands to the workflow and page that supplied their native fields. */
    #dimensionEntries(binding) {
        const targets = [...(binding.width.targets?.() ?? []), ...(binding.height.targets?.() ?? [])];
        return [
            ...this.#canvasDimensionEntries(binding),
            ...(this.#options.dimensionContributions?.(targets) ?? []),
        ];
    }
    /** Bind the peer canvas geometry commands to this binding's workflow scope. */
    #canvasDimensionEntries(binding) {
        const commands = this.#options.dimensions;
        return canvasDimensionMenuEntries(Boolean(commands?.available), async (action, configure, signal) => {
            if (!commands)
                return;
            const cancellation = signal ?? new CanvasCancellation();
            const workflow = this.#options.workflowContext?.();
            this.#dimensionTasks.add(cancellation);
            try {
                await commands.execute(action, {
                    ...binding,
                    current: () => (!this.#options.workflowContext ||
                        (workflow != null && workflow === this.#options.workflowContext())) &&
                        binding.current?.() !== false,
                }, cancellation, configure);
            }
            finally {
                this.#dimensionTasks.delete(cancellation);
            }
        }, (operation) => {
            void operation.catch(this.#options.reportError);
        });
    }
}
