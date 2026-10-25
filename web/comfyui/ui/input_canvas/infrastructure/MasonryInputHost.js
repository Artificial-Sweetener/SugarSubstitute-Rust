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
/** Follow positively registered masonry cards without discovering graphs from DOM mutations. */
import { reconcileNativeNodeBodyAccessories } from '../../surface/NativeNodeBodyAccessories.js';
import { NativeNodeChoiceFields } from '../../surface/NativeNodeChoiceFields.js';
import { HostSurfaceObserver } from '../../core/HostSurfaceObserver.js';
import { readNativeNodeCard } from '../../surface/NativeNodeCardBoundary.js';
import { MasonryInputEditor } from '../presentation/MasonryInputEditor.js';
import { InputLoaderPresentation } from './InputLoaderPresentation.js';
import { refreshNativeNodeCards } from '../../surface/NativeNodeCardRefresh.js';
/** Mount previews only in masonry and leave every unsupported canvas's capabilities unchanged. */
export class MasonryInputHost {
    document;
    loaders;
    previews;
    upload;
    report;
    bindings;
    associations;
    #views = new Map();
    #retired = new WeakMap();
    #observer = null;
    #fields = new WeakMap();
    /** Share the registration catalog and peer capability resolver with execution preparation. */
    constructor(document, loaders, previews, upload, report, bindings, associations) {
        this.document = document;
        this.loaders = loaders;
        this.previews = previews;
        this.upload = upload;
        this.report = report;
        this.bindings = bindings;
        this.associations = associations;
    }
    /** Observe card mounts and scoped native value changes, excluding owned preview rendering. */
    setup() {
        if (this.#observer)
            return;
        this.#observer = new HostSurfaceObserver(this.document.body, '.sugarcubes-cube-face__node-card', (changes) => {
            for (const change of changes)
                this.#reconcile(change.element);
        }, { content: true, exclude: '[data-sc-input-editor],[data-sc-input-controls]' });
        this.document.addEventListener('sugarcubes-canvas-ready', this.#ready);
        this.document.addEventListener('change', this.#fileChanged, true);
        this.document.defaultView?.addEventListener('pagehide', this.dispose);
        this.#ready();
    }
    /** Release live surfaces before Comfy configures another workflow. */
    beforeConfigureGraph() {
        for (const [root, view] of this.#views) {
            this.#retired.set(root, view.node);
            this.#drop(root);
        }
        this.#views.clear();
    }
    /** Retire native observers, preview resources and page-owned listeners. */
    dispose = () => {
        this.beforeConfigureGraph();
        this.#observer?.disconnect();
        this.#observer = null;
        this.previews.dispose();
        this.document.removeEventListener('sugarcubes-canvas-ready', this.#ready);
        this.document.removeEventListener('change', this.#fileChanged, true);
        this.document.defaultView?.removeEventListener('pagehide', this.dispose);
    };
    /** Refresh known mounted surfaces after capability negotiation without inventorying graph candidates. */
    #ready = () => {
        void this.bindings.ready().catch(this.report);
        for (const root of this.document.querySelectorAll('.sugarcubes-workflow-masonry__card .sugarcubes-cube-face__node-card,.sugarcubes-workflow-masonry__card.sugarcubes-cube-face__node-card'))
            this.#reconcile(root);
    };
    /** Resolve exact node ownership and keep all unrelated popup mutations outside this feature. */
    #reconcile(root) {
        try {
            const current = this.#views.get(root);
            if (!root.isConnected || !root.closest('.sugarcubes-workflow-masonry__card')) {
                this.#drop(root);
                return;
            }
            const boundary = readNativeNodeCard(root);
            if (!boundary)
                return;
            if (this.#retired.get(root) === boundary.node)
                return;
            this.#retired.delete(root);
            const node = boundary.node, loaders = this.loaders.all(node.type ?? node.class_type ?? '');
            if (!loaders.length || node.flags?.collapsed === true) {
                this.#drop(root);
                return;
            }
            if (current?.node === node) {
                for (const view of current.editors.values()) {
                    view.binding.adoptFiles();
                    view.editor.refresh();
                }
                this.#place(root);
                if (current.presentation.refresh())
                    refreshNativeNodeCards(this.document, { kind: 'nodes', nodes: [node] });
                return;
            }
            this.#drop(root);
            const presentation = new InputLoaderPresentation(node, root, loaders.map((loader) => loader.field));
            const editors = new Map();
            for (const loader of loaders) {
                const binding = this.bindings.get(node, loader);
                const editor = new MasonryInputEditor(this.document, binding, this.previews, this.upload, this.report, this.#fieldScope(node, loader.field), root, loader.kind === 'mask' ? this.associations.image(node, loader.field) : null, loaders.length > 1);
                root.append(editor.element);
                editors.set(loader.field, { editor, binding });
            }
            this.#views.set(root, { node, editors, presentation });
            this.#place(root);
            refreshNativeNodeCards(this.document, { kind: 'nodes', nodes: [node] });
        }
        catch (error) {
            this.report(error);
        }
    }
    /** Keep previews in native card geometry while fields retain their actual renderer and DOM ownership. */
    #place(root) {
        const editors = [...(this.#views.get(root)?.editors.values() ?? [])].map((view) => view.editor);
        reconcileNativeNodeBodyAccessories(root, editors.map((editor) => editor.element), editors.map((editor) => editor.controlsElement));
    }
    /** Share the native node-field lifecycle across projections of the same exact bound field. */
    #fieldScope(node, field) {
        let scopes = this.#fields.get(node);
        if (!scopes) {
            scopes = new Map();
            this.#fields.set(node, scopes);
        }
        let scope = scopes.get(field);
        if (!scope) {
            scope = new NativeNodeChoiceFields(node, 'canvas-input:' + field, this.document);
            scopes.set(field, scope);
        }
        return scope;
    }
    /** Retire exact replacement leases before asking the native owner to restore its presentation. */
    #drop(root) {
        const view = this.#views.get(root);
        if (!view)
            return;
        this.#views.delete(root);
        for (const item of view.editors.values())
            item.editor.dispose();
        view.presentation.dispose();
        if (root.isConnected)
            refreshNativeNodeCards(this.document, { kind: 'nodes', nodes: [view.node] });
    }
    /** Import only explicit field edits on the owning mounted loader. */
    #fileChanged = (event) => {
        if (!(event.target instanceof Element) ||
            event.target.closest('[data-sc-input-editor],[data-sc-input-controls]'))
            return;
        const root = event.target.closest('.sugarcubes-cube-face__node-card');
        if (!root)
            return;
        const view = this.#views.get(root);
        if (view)
            queueMicrotask(() => {
                try {
                    for (const item of view.editors.values())
                        item.binding.adoptFiles();
                }
                catch (error) {
                    this.report(error);
                }
            });
    };
}
