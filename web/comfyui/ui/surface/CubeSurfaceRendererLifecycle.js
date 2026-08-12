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
/** Own lazy Comfy renderer/runtime acquisition and release. */
import { loadComfyNativeNodeComponent, loadComfyVueRuntime, } from './ComfyRuntimeModuleLoader.js';
import { ComfyVueNodeCardRenderer } from './ComfyVueNodeCardRenderer.js';
import { findComfyNativeNodeMount, findComfyVueAppContext } from './ComfyVueTree.js';
/** Retain renderer state only for the active Comfy Vue application. */
export class CubeSurfaceRendererLifecycle {
    options;
    renderer = null;
    runtime = null;
    createRenderer;
    constructor(options) {
        this.options = options;
        const providedRenderer = options.renderer;
        this.createRenderer =
            options.createRenderer ??
                (providedRenderer ? async () => providedRenderer : () => this.load());
    }
    /** Lazily resolve the native Nodes 2 renderer. */
    getRenderer() {
        this.renderer ??= this.createRenderer();
        return this.renderer;
    }
    /** Lazily resolve the Vue runtime used by native slot layout. */
    getRuntime() {
        this.runtime ??= loadComfyVueRuntime(this.options.document);
        return this.runtime;
    }
    /** Release renderer state tied to a replaced Comfy Vue application. */
    release() {
        const renderer = this.renderer;
        this.renderer = null;
        this.runtime = null;
        if (!renderer)
            return;
        void renderer
            .then((resolved) => resolved.dispose())
            .catch((error) => {
            const reason = error instanceof Error ? error.message : String(error);
            this.options.logger.warn(`SugarCubes failed to dispose a replaced Nodes 2 renderer: ${reason}`, { reason, error });
        });
    }
    /** Dispose renderer state during presenter teardown. */
    dispose() {
        this.release();
    }
    async load() {
        const vueRoot = this.options.document.querySelector('#vue-app');
        if (!vueRoot)
            throw new Error('Comfy Vue application root is not mounted.');
        const appContext = findComfyVueAppContext(vueRoot);
        if (!appContext)
            throw new Error('Comfy Vue application context is not mounted.');
        const mountedComponent = findComfyNativeNodeMount(vueRoot)?.component;
        const [component, runtime] = await Promise.all([
            mountedComponent
                ? Promise.resolve(mountedComponent)
                : loadComfyNativeNodeComponent(this.options.document),
            this.getRuntime(),
        ]);
        return new ComfyVueNodeCardRenderer({ component, appContext, runtime });
    }
}
