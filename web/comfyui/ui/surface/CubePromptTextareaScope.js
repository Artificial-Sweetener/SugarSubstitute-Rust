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
/** Reconcile prompt autosizing within one renderer-owned DOM scope. */
import { CubePromptTextareaAutosizer } from './CubePromptTextareaAutosizer.js';
import { findCubeFacePromptWidget } from './CubeFacePromptPolicy.js';
import { mountPromptEditorSurface, refreshPromptEditorSurface, } from '../prompt_editor/PromptEditorSurface.js';
import { presentCubeCardNode } from './CubeCardWidgetSources.js';
/** Own the set of prompt textareas currently rendered beneath one root. */
export class CubePromptTextareaScope {
    #root;
    #autosizer;
    #bound = new Set();
    #editors = new Map();
    /** Bind one independent native-card DOM scope. */
    constructor(root) {
        this.#root = root;
        this.#autosizer = new CubePromptTextareaAutosizer(root.ownerDocument);
    }
    /** Match autosizing ownership to the current semantic prompt presentation. */
    reconcile(node, sources = []) {
        const presented = findCubeFacePromptWidget(presentCubeCardNode(node, sources));
        const source = sources.find((candidate) => candidate.targetName === presented?.name);
        const owner = source?.node ?? node;
        const widget = source
            ? (owner.widgets?.find((candidate) => candidate.name === source.name) ?? null)
            : presented;
        const desired = new Set(widget
            ? this.#root.querySelectorAll('textarea:not([data-sp-owned])')
            : []);
        for (const [textarea, mounted] of this.#editors) {
            if (desired.has(textarea) && mounted.surface.widget === widget)
                continue;
            mounted.release();
            this.#editors.delete(textarea);
        }
        for (const textarea of [...this.#bound]) {
            if (desired.has(textarea))
                continue;
            this.#autosizer.unbind(textarea);
            this.#bound.delete(textarea);
        }
        for (const textarea of desired) {
            const mounted = this.#editors.get(textarea);
            if (mounted) {
                refreshPromptEditorSurface(mounted.surface);
                continue;
            }
            if (widget && desired.size === 1) {
                const surface = {
                    node: owner,
                    widget,
                    textarea,
                    geometryChanged: () => undefined,
                    hostReadOnly: () => false,
                };
                const release = mountPromptEditorSurface(surface);
                if (release) {
                    this.#autosizer.unbind(textarea);
                    this.#bound.delete(textarea);
                    this.#editors.set(textarea, { surface, release });
                    continue;
                }
            }
            if (this.#bound.has(textarea))
                continue;
            this.#autosizer.bind(textarea);
            this.#bound.add(textarea);
        }
    }
    /** Restore every textarea when its native card unmounts. */
    dispose() {
        for (const mounted of this.#editors.values())
            mounted.release();
        this.#editors.clear();
        this.#autosizer.dispose();
        this.#bound.clear();
    }
}
