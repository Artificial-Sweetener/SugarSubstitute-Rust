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
import { HOST_CAPABILITIES } from '../../core/embedded/PanelRuntime.js';
import { OutputPresenterSelection, canvasRequiresBrowserMedia, } from '../application/OutputPresenterSelection.js';
import { canvasDisplayPlan } from '../application/CanvasDisplayPlan.js';
import { CanvasViewRegistry } from '../application/CanvasViewRegistry.js';
import { CanvasSourceInventory } from '../application/CanvasSourceInventory.js';
import { CanvasSelectionController } from '../application/CanvasSelectionController.js';
import { CanvasInspectionStore } from '../application/CanvasInspectionStore.js';
import { WebCanvasAdapter } from '../application/WebCanvasAdapter.js';
import { CanvasSessionCoordinator } from '../application/CanvasSessionCoordinator.js';
import { CanvasSessionSnapshots } from '../application/CanvasSessionSnapshots.js';
import { windowId } from '../domain/CanvasIdentity.js';
import { isRecord } from '../../types/common.js';
import { NativeCanvasAttachment } from './NativeCanvasAttachment.js';
import { nativeCanvasRendererKey } from './NativeCanvasCapabilities.js';
import { ComfyCanvasSourceInventory } from './ComfyCanvasSourceInventory.js';
import { ComfyCanvasPreferences } from './ComfyCanvasPreferences.js';
import { ComfyCanvasWorkspaceHost } from './ComfyCanvasWorkspaceHost.js';
import { CanvasPrimarySurface } from './CanvasPrimarySurface.js';
import { ComfyCanvasToolbar } from './ComfyCanvasToolbar.js';
import { ensureCanvasWorkspaceStyles } from './CanvasWorkspaceStyles.js';
import { CanvasRevealCoordinator } from '../application/CanvasRevealCoordinator.js';
import { ComfyImageRevealHost } from './ComfyImageRevealHost.js';
import { BrowserOutputTransfer } from '../../output_saving/BrowserOutputTransfer.js';
import { BrowserImageClipboard } from '../../output_saving/BrowserImageClipboard.js';
import { BrowserImageDownload } from '../../output_saving/BrowserImageDownload.js';
import { BrowserImageRendition } from '../../output_saving/BrowserImageRendition.js';
import { canvasOutputTransfers } from './CanvasOutputTransfers.js';
import { NativeSourceSavingHost } from '../../output_saving/NativeSourceSavingHost.js';
/** Keep source, selection, resource, frame, settings, and execution ownership explicit in a thin composition. */
export class ComfyCanvasHost {
    options;
    /** Share the current view and camera owners with the desktop session composition. */
    sessionSnapshots;
    #sources;
    #adapter;
    #coordinator;
    #preferences;
    #workspace;
    #primary;
    #toolbar;
    #reveal;
    #imageActions;
    #saving;
    #transfers;
    #browser = null;
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
        const transfers = (this.#transfers = new BrowserOutputTransfer(document.location.origin, (input, init) => window.fetch(input, init), new BrowserImageClipboard(document), new BrowserImageDownload(document), new BrowserImageRendition(document)));
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
        this.#adapter = new WebCanvasAdapter({
            wants: (frame) => this.#primary.wants(frame.source),
            receive: (frame) => this.#primary.receive(frame.preview, frame.bytes),
        });
        this.#coordinator = new CanvasSessionCoordinator(execution.sessions, this.#adapter, inventory, views, selection, this.#sources);
        this.sessionSnapshots = new CanvasSessionSnapshots(this.#coordinator, views, selection, inspections);
        this.#stops.push(this.#coordinator.onRetired((view) => {
            inspections.remove(view);
            this.#browser?.remove(view);
        }));
        this.#preferences = new ComfyCanvasPreferences(app);
        const changed = () => {
            if (this.#started && !this.#disposed) {
                this.#primary.reconcile();
                this.#toolbar.update(this.#workspace.visible, this.#workspace.content !== null);
            }
        };
        this.#workspace = new ComfyCanvasWorkspaceHost(app, document, options.defaultEditorWidth, frames, changed, report, options.presentation);
        const presenter = new OutputPresenterSelection({
            owner: () => {
                const host = options.host.read();
                if (!host)
                    return 'pending';
                return host.declares(HOST_CAPABILITIES.hostOutputSurface) ? 'host' : 'page';
            },
            surface: () => execution.canvases.surfaceHost('embedded-document'),
            discovered: () => execution.canvasSurfaceDiscovered,
            browserMedia: () => {
                if (options.route?.read()?.route === 'input')
                    return false;
                const view = this.#coordinator.active();
                const catalog = view ? this.#coordinator.catalog(view) : null;
                return (!!view &&
                    !!catalog &&
                    canvasRequiresBrowserMedia(canvasDisplayPlan(views.read(view), catalog, selection.items(view, catalog), selection.preview(view, catalog))));
            },
        });
        const nativePresenter = () => presenter.current() !== 'browser';
        this.#primary = new CanvasPrimarySurface({
            document,
            window: windowId(nextKey()),
            views,
            coordinator: this.#coordinator,
            element: () => this.#workspace.canvas,
            renderer: () => `${resolveComfyRendererMode(app, null, document)}:${nativePresenter() ? `${nativeCanvasRendererKey(execution.canvases.surfaceHost('embedded-document'))}:${presenter.current()}` : 'browser'}`,
            retireBeforeMount: (previous, next) => previous.endsWith(':browser') !== next.endsWith(':browser'),
            report,
            mount: (element, address, _catalog, accepts, acceptsPreparation) => {
                const shared = {
                    address,
                    views,
                    selection,
                    inspections,
                    clock,
                    frames,
                    accepts,
                    catalog: () => this.#coordinator.catalog(address.viewId),
                    connected: () => execution.connected,
                    ...canvasOutputTransfers(transfers),
                    transferChanged: (listener) => {
                        const profile = transfers.subscribe(listener);
                        const settings = saving?.configuration.subscribe(listener);
                        return () => {
                            profile();
                            settings?.();
                        };
                    },
                    report: (error) => report(error),
                    ...(options.route ? { route: options.route } : {}),
                };
                if (nativePresenter())
                    return new NativeCanvasAttachment(element, {
                        ...shared,
                        acceptsPreparation,
                        nativeSurface: () => execution.canvases.surfaceHost('embedded-document'),
                        surfacePending: () => presenter.current() === 'pending',
                        editing: () => execution.canvases.editHost(),
                    });
                return import('./ComfyBrowserCanvasHost.mjs').then(({ ComfyBrowserCanvasHost }) => {
                    if (!acceptsPreparation())
                        return null;
                    const browser = (this.#browser ??= new ComfyBrowserCanvasHost(document, logger, options.memory));
                    return browser.mount(element, shared);
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
        return {
            ...this.#primary.diagnostics(),
            inventory: this.#sources.diagnostics(),
            ...(this.#browser?.diagnostics() ?? {
                posters: 0,
                posterBytes: 0,
                parkedImages: 0,
                parkedBytes: 0,
            }),
        };
    }
    /** Install one source adapter, catalog coordinator, preview consumer, and primary attachment. */
    setup() {
        if (this.#started || this.#disposed)
            return;
        try {
            this.#preferences.setup();
            this.#sources.install();
            this.#stops.push(this.options.execution.canvases.register(this.#adapter.adapter));
            this.#coordinator.install();
            this.#started = true;
            void this.#transfers.refresh().catch(this.options.report);
            if (this.options.saving)
                this.#stops.push(this.options.saving.configuration.subscribe(() => {
                    void this.#transfers.refresh().catch(this.options.report);
                }));
            ensureCanvasWorkspaceStyles(this.options.document);
            this.#primary.setup();
            this.#workspace.setup();
            this.#toolbar.setup();
            this.#imageActions.setup();
            this.#saving?.setup();
            this.#stops.push(this.options.execution.subscribeConnection(() => this.#primary.reconcile()));
            this.#stops.push(this.options.host.subscribe(() => this.#primary.reconcile()));
            if (this.options.route)
                this.#stops.push(this.options.route.subscribe(() => this.#primary.reconcile()));
            this.options.document.addEventListener('sugarcubes-canvas-discovered', this.#canvasDiscovered);
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
        this.options.document.removeEventListener('sugarcubes-canvas-discovered', this.#canvasDiscovered);
        this.#primary.dispose();
        this.#imageActions.dispose();
        this.#saving?.dispose();
        this.#workspace.dispose();
        this.#toolbar.dispose();
        this.sessionSnapshots.dispose();
        this.#coordinator.dispose();
        for (const stop of this.#stops.splice(0))
            stop();
        this.#adapter.dispose();
        this.#sources.dispose();
        this.#transfers.dispose();
        this.#browser?.dispose();
    }
    /** Honor actual page teardown without introducing a recurring visibility or frame loop. */
    #pagehide = () => this.dispose();
    /** Reselect the presenter and surface after every concluded native discovery. */
    #canvasDiscovered = () => this.#primary.reconcile();
}
