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
/** Reconcile focused Cube adapters when Comfy remounts selection surfaces. */
/** Own selection resolution and mutation-observer lifecycle without rendering feature UI. */
export class ComfyCubeSelectionSurfaceAdapter {
    #document;
    #canvas;
    #contexts;
    #toolbox;
    #menus;
    #observer;
    #scheduled = false;
    /** Bind selection semantics to focused host-surface adapters. */
    constructor(options) {
        this.#document = options.document;
        this.#canvas = options.canvas;
        this.#contexts = options.contexts;
        this.#toolbox = options.toolbox;
        this.#menus = options.menus;
        this.#observer = new MutationObserver(() => this.#schedule());
    }
    /** Observe Vue mounts and renderer switches, then reconcile immediately. */
    install() {
        this.#observer.observe(this.#document.body, { childList: true, subtree: true });
        this.refresh();
    }
    /** Restore presentation mutations when Comfy replaces the Cube runtime. */
    dispose() {
        this.#observer.disconnect();
        this.#toolbox.dispose();
        this.#menus.clear();
    }
    /** Reconcile each focused host surface from one semantic selection snapshot. */
    refresh() {
        this.#scheduled = false;
        const selection = this.#contexts.resolveSelection(this.#canvas.selectedItems);
        const cube = selection.isSingleCube ? (selection.cubeNodes[0] ?? null) : null;
        this.#toolbox.present(cube);
        this.#menus.present(selection.containsCube);
    }
    /** Coalesce mutation bursts produced by PrimeVue menu mounting. */
    #schedule() {
        if (this.#scheduled)
            return;
        this.#scheduled = true;
        queueMicrotask(() => this.refresh());
    }
}
