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
/** Compose the workflow-owned browser canvas without extending Comfy's execution or rendering authorities. */
import { resolveComfyRendererMode } from '../../core/ComfyRendererMode.js';
import { CanvasViewRegistry } from '../application/CanvasViewRegistry.js';
import { CanvasSourceInventory } from '../application/CanvasSourceInventory.js';
import { CanvasSelectionController } from '../application/CanvasSelectionController.js';
import { CanvasInspectionStore } from '../application/CanvasInspectionStore.js';
import { CanvasImageAllowances } from '../application/CanvasImageAllowances.js';
import { CanvasSessionCoordinator } from '../application/CanvasSessionCoordinator.js';
import { windowId } from '../domain/CanvasIdentity.js';
import { isRecord } from '../../types/common.js';
import { BrowserCanvasAttachment } from './BrowserCanvasAttachment.js';
import { BrowserCanvasImageLoader } from './BrowserCanvasImageLoader.js';
import { ComfyCanvasSourceInventory } from './ComfyCanvasSourceInventory.js';
import { ComfyCanvasPreferences } from './ComfyCanvasPreferences.js';
import { ComfyCanvasWorkspaceHost } from './ComfyCanvasWorkspaceHost.js';
import { CanvasPrimarySurface } from './CanvasPrimarySurface.js';
import { ComfyCanvasToolbar } from './ComfyCanvasToolbar.js';
import { comfyCanvasMediaUrl, openComfyCanvasMedia } from './ComfyCanvasMedia.js';
import { ensureBrowserCanvasWorkspaceStyles } from './BrowserCanvasWorkspaceStyles.js';
/** Keep source, selection, resource, frame, settings, and execution ownership explicit in a thin composition. */
export class ComfyCanvasHost {
    options;
    #sources;
    #coordinator;
    #preferences;
    #workspace;
    #primary;
    #toolbar;
    #stops = [];
    #started = false;
    #disposed = false;
    /** Allocate lightweight owners immediately and defer public host access until extension setup. */
    constructor(options) {
        this.options = options;
        const { document, execution, app, logger, report } = options;
        const window = document.defaultView;
        if (!window)
            throw new Error('Output canvas window is unavailable.');
        const nextKey = () => [...window.crypto.getRandomValues(new Uint32Array(4))]
            .map((word) => word.toString(16).padStart(8, '0'))
            .join('');
        const frames = {
            now: () => window.performance.now(),
            request: (callback) => {
                const id = window.requestAnimationFrame(callback);
                return () => window.cancelAnimationFrame(id);
            },
        };
        const clock = {
            now: () => window.performance.now(),
            later: (delay, callback) => {
                const id = window.setTimeout(callback, delay);
                return () => window.clearTimeout(id);
            },
        };
        const views = new CanvasViewRegistry(nextKey);
        const selection = new CanvasSelectionController(views);
        const inspections = new CanvasInspectionStore();
        const allowances = new CanvasImageAllowances();
        const inventory = new CanvasSourceInventory();
        this.#sources = new ComfyCanvasSourceInventory(execution.sessions, inventory, () => (isRecord(app) ? app.graph : null), logger);
        this.#coordinator = new CanvasSessionCoordinator(execution.sessions, execution.results, inventory, views, selection, this.#sources);
        this.#stops.push(this.#coordinator.onRetired((view) => {
            inspections.remove(view);
            allowances.remove(view);
        }));
        this.#preferences = new ComfyCanvasPreferences(app);
        const changed = () => {
            if (this.#started && !this.#disposed) {
                this.#primary.reconcile();
                this.#toolbar.update(this.#workspace.canvas !== null, this.#workspace.content !== null);
            }
        };
        this.#workspace = new ComfyCanvasWorkspaceHost(app, document, this.#preferences, frames, changed, report);
        this.#primary = new CanvasPrimarySurface({
            document,
            window: windowId(nextKey()),
            views,
            coordinator: this.#coordinator,
            element: () => this.#workspace.canvas,
            renderer: () => resolveComfyRendererMode(app, null, document),
            report,
            mount: (element, address, _catalog, accepts) => {
                const loader = new BrowserCanvasImageLoader(document, {
                    create: (blob) => window.URL.createObjectURL(blob),
                    revoke: (url) => window.URL.revokeObjectURL(url),
                });
                return new BrowserCanvasAttachment(element, {
                    address,
                    views,
                    selection,
                    inspections,
                    allowances,
                    clock,
                    frames,
                    loader,
                    logger,
                    accepts,
                    catalog: () => this.#coordinator.catalog(address.viewId),
                    connected: () => execution.connected,
                    url: (item) => comfyCanvasMediaUrl(document.location.origin, item),
                    open: (item) => openComfyCanvasMedia(window, item),
                    disposeLoader: () => loader.dispose(),
                    report: (error) => report(error),
                });
            },
        });
        this.#toolbar = new ComfyCanvasToolbar(document, () => this.toggle());
    }
    /** Supply the actual remaining workspace to masonry even while the output pane is collapsed. */
    get content() {
        return this.#workspace.content;
    }
    /** Expose explicit primary-view resource accounting through the established debug surface. */
    diagnostics() {
        return { ...this.#primary.diagnostics(), inventory: this.#sources.diagnostics() };
    }
    /** Install one source adapter, catalog coordinator, preview consumer, and primary attachment. */
    setup() {
        if (this.#started || this.#disposed)
            return;
        try {
            this.#preferences.setup();
            this.#sources.install();
            this.#coordinator.install();
            this.#started = true;
            ensureBrowserCanvasWorkspaceStyles(this.options.document);
            this.#primary.setup();
            this.#workspace.setup();
            this.#toolbar.setup();
            this.#stops.push(this.options.execution.previews.subscribe(this.#primary), this.options.execution.subscribeConnection(() => this.#primary.reconcile()));
            this.options.document.defaultView?.addEventListener('pagehide', this.#pagehide);
        }
        catch (error) {
            this.dispose();
            this.options.report(error);
        }
    }
    /** Use the same command owner in graph and masonry presentation. */
    toggle() {
        this.#workspace.toggle();
    }
    /** Release primary pixels before graph replacement while preserving open workflow view state. */
    beforeConfigureGraph() {
        this.#workspace.beforeConfigureGraph();
    }
    /** Restore a qualified allocation before masonry resumes borrowed native controls. */
    afterConfigureGraph() {
        this.#workspace.afterConfigureGraph();
    }
    /** Dispose this browser composition without disposing shared execution or other future window owners. */
    dispose() {
        if (this.#disposed)
            return;
        this.#disposed = true;
        this.options.document.defaultView?.removeEventListener('pagehide', this.#pagehide);
        this.#primary.dispose();
        this.#workspace.dispose();
        this.#toolbar.dispose();
        this.#coordinator.dispose();
        for (const stop of this.#stops.splice(0))
            stop();
        this.#sources.dispose();
        this.#preferences.dispose();
    }
    /** Honor actual page teardown without introducing a recurring visibility or frame loop. */
    #pagehide = () => this.dispose();
}
