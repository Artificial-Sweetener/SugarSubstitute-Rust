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
import { CanvasRevealCoordinator } from '../application/CanvasRevealCoordinator.js';
import { ComfyImageRevealHost } from './ComfyImageRevealHost.js';
import { BrowserOutputTransfer } from '../../output_saving/BrowserOutputTransfer.js';
import { BrowserImageClipboard } from '../../output_saving/BrowserImageClipboard.js';
import { NativeSourceSavingHost } from '../../output_saving/NativeSourceSavingHost.js';
/** Keep source, selection, resource, frame, settings, and execution ownership explicit in a thin composition. */
export class ComfyCanvasHost {
    options;
    #sources;
    #coordinator;
    #preferences;
    #workspace;
    #primary;
    #toolbar;
    #reveal;
    #imageActions;
    #saving;
    #stops = [];
    #started = false;
    #disposed = false;
    /** Allocate lightweight owners immediately and defer public host access until extension setup. */
    constructor(options) {
        this.options = options;
        const { document, execution, app, logger, report, saving } = options;
        const window = document.defaultView;
        if (!window)
            throw new Error('Output canvas window is unavailable.');
        const transfers = new BrowserOutputTransfer(document.location.origin, (input, init) => window.fetch(input, init), new BrowserImageClipboard(document));
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
        const selection = new CanvasSelectionController(views, {
            latestOrder: (session) => execution.latestSubmissionOrder(session),
        });
        const inspections = new CanvasInspectionStore();
        const allowances = new CanvasImageAllowances();
        const inventory = new CanvasSourceInventory();
        this.#sources = new ComfyCanvasSourceInventory(execution.sessions, inventory, () => (isRecord(app) ? app.graph : null), logger);
        this.#saving = options.saving
            ? new NativeSourceSavingHost(options.saving, {
                root: () => (isRecord(app) ? app.graph : null),
                canvas: () => (isRecord(app) ? app.canvas : null),
                scope: () => execution.sessions.read().active,
                sources: () => this.#sources.savingSources(),
            }, report, { document, subscribe: (listener) => this.#sources.subscribe(listener) })
            : null;
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
        this.#workspace = new ComfyCanvasWorkspaceHost(app, document, this.#preferences, frames, changed, report, options.presentation);
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
                    copy: (item, accepts) => transfers.copy(item.file, accepts),
                    drag: (item, signal) => transfers.drag(item.file, signal),
                    ...(saving
                        ? {
                            transferChanged: (listener) => saving.configuration.subscribe(listener),
                        }
                        : {}),
                    disposeLoader: () => loader.dispose(),
                    report: (error) => report(error),
                });
            },
        });
        this.#toolbar = new ComfyCanvasToolbar(document, () => this.toggle());
        this.#reveal = new CanvasRevealCoordinator({
            read: () => {
                const viewId = this.#coordinator.active();
                const catalog = viewId ? this.#coordinator.catalog(viewId) : null;
                return viewId && catalog ? { viewId, catalog } : null;
            },
            open: (view) => {
                this.#workspace.show();
                return this.#primary.destination(view);
            },
        });
        this.#imageActions = new ComfyImageRevealHost({
            document,
            app,
            logger,
            catalog: () => {
                const view = this.#coordinator.active();
                return view ? this.#coordinator.catalog(view) : null;
            },
            sources: () => this.#sources.nativeSources(),
            subscribe: (listener) => this.#coordinator.subscribe(() => listener()),
            reveal: (target) => this.reveal(target),
        });
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
            this.#imageActions.setup();
            this.#saving?.setup();
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
    /** Reveal a provenance-qualified completed thumbnail through the shared canvas authority. */
    async reveal(target) {
        if (this.#disposed || !this.#started)
            return false;
        return await this.#reveal.reveal(target);
    }
    /** Preserve qualified history pixels and retire primary resources before graph replacement. */
    beforeConfigureGraph() {
        const loading = this.options.execution.sessions.read().loading;
        this.#workspace.beforeConfigureGraph(loading && this.#coordinator.active() !== null);
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
        this.#imageActions.dispose();
        this.#saving?.dispose();
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
