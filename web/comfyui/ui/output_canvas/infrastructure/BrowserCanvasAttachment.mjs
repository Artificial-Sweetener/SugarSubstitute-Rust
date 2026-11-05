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
/** Compose a qualified browser image surface beneath reusable, renderer-independent Vue controls. */
import { CanvasCameraController } from '../application/CanvasCameraController.mjs';
import { CanvasImageBudget } from '../application/CanvasImageBudget.mjs';
import { CanvasImageDecodeScheduler } from '../application/CanvasImageDecodeScheduler.mjs';
import { CanvasImageResources } from '../application/CanvasImageResources.mjs';
import { CanvasStatusPublisher } from '../application/CanvasStatusPublisher.js';
import { CanvasSourceNavigation } from '../application/CanvasSourceNavigation.js';
import { CanvasViewProjection } from '../application/CanvasViewProjection.js';
import { inspectBrowserCanvasFrame } from './BrowserCanvasInspection.mjs';
import { sameCanvasNavigation } from '../domain/CanvasViewState.js';
import { BrowserCanvasImages } from '../presentation/browser/BrowserCanvasImages.mjs';
import { BrowserCanvasInput } from '../presentation/browser/BrowserCanvasInput.mjs';
import { CanvasViewportKeyboard } from '../presentation/CanvasViewportKeyboard.js';
import { BrowserCanvasOverview } from '../presentation/browser/BrowserCanvasOverview.mjs';
import { browserCanvasSnapshot } from '../presentation/browser/BrowserCanvasSnapshot.mjs';
import { mountCanvasOverlay, } from '../presentation/vue/CanvasOverlayRuntime.js';
import { CanvasSurfaceGeometryObserver } from './CanvasSurfaceGeometryObserver.js';
import { CanvasSurfacePort } from './CanvasSurfacePort.js';
import { BrowserCanvasResources } from './BrowserCanvasResources.mjs';
import { applyCanvasAction } from '../application/CanvasActions.js';
import { BrowserCanvasActionSurface } from './BrowserCanvasActionSurface.mjs';
import { ensureBrowserCanvasStyles } from './BrowserCanvasStyles.mjs';
import { CanvasOutputDrag } from '../presentation/CanvasOutputDrag.js';
import { BrowserCanvasDragSubjects } from './BrowserCanvasDragSubject.mjs';
import { CanvasMediaActions } from './CanvasMediaActions.js';
import { BrowserCanvasMediaDetail } from './BrowserCanvasMediaDetail.mjs';
import { BrowserCanvasMediaSessions } from './BrowserCanvasMediaSessions.mjs';
import { ComfyCanvasModelRuntime } from './ComfyCanvasModelRuntime.mjs';
import { isRecord } from '../../types/common.js';
/** Keep composition thin: navigation, cameras, admissions, input, and DOM each retain their own owner. */
export class BrowserCanvasAttachment {
    element;
    options;
    viewport;
    port;
    #images;
    #overview;
    #input;
    #stopKeyboard;
    #drag;
    #media;
    #detail;
    #stopTransfer;
    #geometry;
    #camera;
    #browserResources;
    #status;
    #viewProjection;
    #overlay;
    #stopView;
    #releaseMemory;
    #catalog;
    #sources;
    #sourceSignature;
    #plan;
    #surface = null;
    #signature = '';
    #linked = true;
    #ready = false;
    #disposed = false;
    /** Mount already-qualified owners into an explicitly allocated workspace element. */
    constructor(element, options) {
        this.element = element;
        this.options = options;
        const catalog = options.catalog();
        if (!catalog || catalog.sessionId !== options.address.sessionId)
            throw new Error('Canvas workflow catalog is unavailable.');
        this.#catalog = catalog;
        this.#sources = Object.freeze(catalog.entries.map((entry) => entry.summary));
        this.#sourceSignature = JSON.stringify(this.#sources);
        this.#viewProjection = new CanvasViewProjection(options.address.viewId, options.views, options.selection, options.inspections);
        this.#plan = this.#viewProjection.project(catalog);
        const document = element.ownerDocument;
        ensureBrowserCanvasStyles(document);
        this.viewport = document.createElement('div');
        // The browser canvas pans its own pixels, so only its viewport shows the grab cursor.
        this.viewport.className = 'sc-output-viewport sc-output-browser';
        this.viewport.setAttribute('data-sc-canvas-geometry', '');
        const overlay = document.createElement('div');
        overlay.className = 'sc-output-overlay-root';
        element.append(this.viewport, overlay);
        this.#images = new BrowserCanvasImages(this.viewport, options.frames);
        this.#detail = new BrowserCanvasMediaDetail(this.viewport, {
            models: options.models ??
                new ComfyCanvasModelRuntime(() => isRecord(document.defaultView) ? document.defaultView.comfyAPI : null),
            sessions: options.mediaSessions ?? new BrowserCanvasMediaSessions(),
            clock: options.clock,
            url: options.url,
            changed: () => {
                if (this.#ready)
                    this.#status.changed(true);
            },
            report: (error) => options.logger.error('SugarCubes output media could not be displayed.', {
                error,
                itemId: this.#plan.primary?.item?.id,
                sourceId: this.#plan.primary?.item?.provenance.sourceId,
            }),
        });
        this.#overview = new BrowserCanvasOverview(element, options.frames, (cells) => this.#browserResources.overview(cells), (cell) => {
            if (!this.#disposed && options.accepts())
                options.selection.apply(options.address.viewId, this.#catalog, cell.preview
                    ? { type: 'select-preview', sourceId: cell.selection.sourceId }
                    : { type: 'select', selection: cell.selection });
        });
        const budget = new CanvasImageBudget();
        this.#releaseMemory = options.memory.register(budget.memoryConsumer(`output-canvas-displayed:${options.address.attachmentId}`));
        const decodes = new CanvasImageDecodeScheduler(options.loader, options.clock);
        const resources = new CanvasImageResources({
            clock: options.clock,
            budget,
            decodes,
            changed: () => this.#resourcesChanged(),
            logger: options.logger,
        });
        this.#browserResources = new BrowserCanvasResources(resources, budget, options.allowances, options.url);
        const dragSubjects = new BrowserCanvasDragSubjects(this.viewport, this.#images, this.#overview, () => this.#plan);
        this.#drag = new CanvasOutputDrag(element, {
            resolve: (target, event) => dragSubjects.resolve(target, event),
            prepare: (key, signal) => {
                const item = dragSubjects.item(key);
                return item && options.drag ? options.drag(item, signal) : Promise.resolve(null);
            },
            accepts: () => !this.#disposed && this.#surface?.visible === true && options.accepts() && !!options.drag,
            report: (error) => options.report(error, null),
        });
        this.#stopTransfer =
            options.transferChanged?.(() => {
                this.#drag.invalidate();
                if (this.#ready)
                    this.#publish();
            }) ?? (() => { });
        this.#status = new CanvasStatusPublisher(options.clock, () => this.#publish());
        this.#camera = new CanvasCameraController({
            view: options.address.viewId,
            inspections: options.inspections,
            clock: options.frames,
            reducedMotion: () => Boolean(document.defaultView?.matchMedia?.('(prefers-reduced-motion: reduce)').matches),
            present: (target) => {
                this.#images.camera(target);
                this.#drag.reconcile();
                if (this.#ready)
                    this.#status.changed();
            },
        });
        const sources = new CanvasSourceNavigation({
            resources: this.#browserResources,
            selection: options.selection,
            read: () => !this.#disposed && options.accepts() ? { plan: this.#plan, catalog: this.#catalog } : null,
            cells: (catalog) => this.#overview.cells(catalog),
        });
        this.#media = new CanvasMediaActions({
            read: () => this.#snapshot(),
            accepts: () => !this.#disposed && options.accepts(),
            ...(options.copy ? { copy: options.copy } : {}),
            ...(options.fileAction ? { fileAction: options.fileAction } : {}),
            ...(options.mediaProfile ? { profile: options.mediaProfile } : {}),
            ...(options.photoshop ? { photoshop: options.photoshop } : {}),
            entries: () => this.#catalog.entries,
        });
        const actionSurface = new BrowserCanvasActionSurface(this.#camera, this.#browserResources, this.#media, options.open);
        this.port = new CanvasSurfacePort(this.#snapshot(), {
            origin: document.location.origin,
            capabilities: {
                schema: 1,
                commands: [
                    'reveal',
                    'select',
                    'select-source',
                    'select-preview',
                    'overview',
                    'batch',
                    'follow',
                    'link',
                    'fit',
                    'native',
                    'zoom',
                    'pan',
                    'comparison',
                    'select-comparison',
                    'swap',
                    'wipe',
                    'retry',
                    'admit-large-image',
                    'open-media',
                    ...this.#media.commands(),
                ],
            },
            accepts: () => !this.#disposed && options.accepts(),
            apply: (command, scope) => {
                if (command.type === 'retry' && this.#detail.status()) {
                    sources.cancel();
                    if (!scope.current())
                        return false;
                    this.#detail.retry(this.#plan, this.#surface?.visible === true);
                    return true;
                }
                return applyCanvasAction(command, {
                    plan: this.#plan,
                    catalog: this.#catalog,
                    selection: options.selection,
                    sources,
                    snapshot: command.type === 'comparison' ? this.#snapshot() : this.port.read(),
                    surface: actionSurface,
                    scope,
                });
            },
            geometry: (geometry) => this.#resize(geometry),
            release: () => this.dispose(),
            report: options.report,
        });
        const cleanup = [
            this.#releaseMemory,
            () => this.#drag.dispose(),
            () => this.#media.dispose(),
            this.#stopTransfer,
            () => this.#images.dispose(),
            () => this.#detail.dispose(),
            () => this.#overview.dispose(),
            () => this.#browserResources.dispose(),
            () => this.#status.dispose(),
            () => this.#camera.dispose(),
            () => this.port.dispose(),
        ];
        try {
            this.#overlay = mountCanvasOverlay(overlay, this.port, options.report, element, (target) => this.#overview.contextTarget(target) ?? this.#images.contextTarget(target) ?? 'primary', options.route ?? null);
            cleanup.push(() => this.#overlay.dispose());
            this.#input = new BrowserCanvasInput(this.viewport, {
                navigable: () => this.#camera.navigable(),
                outbound: (event) => this.#drag.allows(event),
                contains: (point) => !this.#detail.status() && this.#images.contains(point),
                interrupt: () => this.#camera.interrupt(),
                pan: (delta) => {
                    this.#camera.pan(delta);
                },
                wheel: (delta, units, anchor) => {
                    this.#camera.wheel(delta, units, anchor);
                },
                pinch: (factor, anchor, delta) => {
                    this.#camera.pinch(factor, anchor, delta);
                },
                doubleClick: (anchor) => {
                    this.#camera.doubleClick(anchor);
                },
            });
            const keyboard = new CanvasViewportKeyboard(this.viewport, this.port, options.report, () => this.#input.cancel());
            cleanup.push(() => keyboard.dispose());
            this.#stopKeyboard = () => keyboard.dispose();
            cleanup.push(() => this.#input.dispose());
            this.#geometry = new CanvasSurfaceGeometryObserver(this.viewport, options.frames, (geometry) => this.port.updateSurface(options.address, geometry));
            cleanup.push(() => this.#geometry.dispose());
            this.#stopView = options.views.subscribe(options.address.viewId, (view) => {
                if (sameCanvasNavigation(this.#plan.view, view)) {
                    const orientationChanged = this.#plan.view.comparison?.orientation !== view.comparison?.orientation;
                    this.#plan = Object.freeze({ ...this.#plan, view });
                    this.#images.wipe(view.comparison);
                    this.#status.changed(orientationChanged);
                }
                else
                    this.refresh();
            });
            cleanup.push(this.#stopView);
            this.#ready = true;
            this.#resize(this.#geometry.read());
            this.refresh();
        }
        catch (error) {
            this.#ready = false;
            this.#disposed = true;
            for (const stop of cleanup.reverse())
                stop();
            options.disposeLoader();
            options.views.releaseAttachment(options.address);
            element.replaceChildren();
            throw error;
        }
    }
    /** Accept graph-thumbnail inspection through the same preparation path as surface commands. */
    async reveal(target) {
        if (this.#disposed || !this.options.accepts())
            return false;
        const request = this.port.createRequest({ type: 'reveal', target }, this.port.read().revision);
        return (await this.port.dispatch(request)).status === 'applied';
    }
    /** Refresh light catalog state at a scoped execution/metadata/navigation boundary. */
    refresh() {
        if (!this.#ready || this.#disposed || !this.options.accepts())
            return;
        if (!this.#surface?.visible)
            this.port.updateSurface(this.options.address, this.#geometry.read());
        const catalog = this.options.catalog();
        if (!catalog || catalog.generation !== this.#catalog.generation)
            return;
        this.#catalog = catalog;
        const sources = catalog.entries.map((entry) => entry.summary);
        const sourceSignature = JSON.stringify(sources);
        if (sourceSignature !== this.#sourceSignature) {
            this.#sources = Object.freeze(sources);
            this.#sourceSignature = sourceSignature;
        }
        this.#plan = this.#viewProjection.project(catalog);
        this.#drag.reconcile();
        const linked = this.#plan.view.linked;
        if (linked !== this.#linked) {
            this.#linked = linked;
            this.#camera.link(linked);
        }
        this.#images.wipe(this.#plan.view.comparison);
        this.#camera.comparison(Boolean(this.#plan.view.comparison));
        const overview = this.#plan.view.mode === 'overview';
        this.#detail.reconcile(this.#plan, this.#surface?.visible === true);
        this.element.classList.toggle('sc-output-overview-mode', overview);
        this.#browserResources.reconcile(this.#plan);
        this.#overview.reconcile({ ...catalog, entries: this.#plan.grid }, overview);
        if (this.#detail.status())
            this.#resourcesChanged();
        const view = this.#plan.view;
        const signature = JSON.stringify([
            view.selection,
            view.sourcePreview,
            view.follow,
            view.mode,
            view.linked,
            view.comparison?.secondary,
            view.comparison?.orientation,
            this.#plan.primary?.item?.id,
            this.#plan.secondary?.item?.id,
            this.#sourceSignature,
            catalog.result.runStatus,
            catalog.result.recovery,
            this.options.connected(),
        ]);
        if (signature !== this.#signature) {
            this.#signature = signature;
            this.#status.changed(true);
        }
        else
            this.#status.changed();
    }
    /** Admit only bytes qualified to this visible attachment's current source and workflow. */
    receive(preview, blob) {
        if (this.wants(preview.sessionId, preview.generation, preview.sourceId)) {
            this.#browserResources.receive(preview, blob);
        }
    }
    /** Refuse byte delivery while hidden, retired, or inspecting an exact batch/comparison. */
    wants(session, generation, source) {
        return (!this.#disposed &&
            this.#surface?.visible === true &&
            this.options.accepts() &&
            this.#catalog.result.context.sessionId === session &&
            this.#catalog.generation === generation &&
            this.options.selection.allowsSourcePreview(this.options.address.viewId, this.#catalog, source));
    }
    /** Count this attachment's owned resources and frames at explicit diagnostic boundaries. */
    diagnostics() {
        return {
            ...this.#browserResources.resources.diagnostics(),
            renderFrames: this.#images.pendingFrames,
            ...this.#detail.diagnostics(),
        };
    }
    /** Retire local presentation before canceling platform images and releasing the attachment lease. */
    dispose() {
        if (this.#disposed)
            return;
        this.#disposed = true;
        this.#ready = false;
        this.#stopView();
        this.#status.dispose();
        this.port.dispose();
        this.#stopKeyboard();
        this.#input.dispose();
        this.#drag.dispose();
        this.#media.dispose();
        this.#stopTransfer();
        this.#geometry.dispose();
        this.#camera.dispose();
        this.#overlay.dispose();
        this.#images.dispose();
        this.#detail.dispose();
        this.#overview.dispose();
        this.#browserResources.dispose();
        this.#releaseMemory();
        this.options.disposeLoader();
        this.options.views.releaseAttachment(this.options.address);
        this.element.replaceChildren();
        this.element.classList.remove('sc-output-overview-mode');
    }
    /** Reconcile an actual decoded route, including the outgoing image during pending replacement. */
    #resourcesChanged() {
        if (!this.#ready || this.#disposed)
            return;
        this.#browserResources.measured();
        const images = this.#browserResources.resources.read();
        const primary = images.find((slot) => slot.slot === 'primary')?.frame ?? null;
        const secondary = images.find((slot) => slot.slot === 'secondary')?.frame ?? null;
        this.#overview.images(images);
        if (this.#plan.view.mode === 'overview' || this.#detail.status()) {
            this.#images.images(null, null);
            this.#drag.reconcile();
            this.#input.cancel();
            this.#camera.suspend();
            this.#status.changed(Boolean(primary?.preview) !== this.port.read().preview);
            return;
        }
        this.#images.images(primary, secondary);
        this.#drag.reconcile();
        if (!inspectBrowserCanvasFrame(this.#camera, primary ?? secondary, this.#surface) ||
            !this.#camera.navigable())
            this.#input.cancel();
        this.#status.changed(Boolean(primary?.preview) !== this.port.read().preview);
    }
    /** Apply settled geometry while suspending all image admissions for zero-sized/hidden allocations. */
    #resize(surface) {
        this.#surface = surface;
        this.#drag.reconcile();
        this.#browserResources.active(surface.visible);
        this.#detail.reconcile(this.#plan, surface.visible);
        this.#detail.resize();
        this.#overview.resize();
        this.#resourcesChanged();
    }
    /** Publish the current actual display at bounded cadence, without graph access. */
    #publish() {
        if (this.#ready && !this.#disposed) {
            this.#media.reconcile();
            this.port.publish(this.#snapshot());
        }
    }
    /** Report actual camera/fidelity rather than the incoming selection's unshown dimensions. */
    #snapshot() {
        return browserCanvasSnapshot(this.options.address, this.#plan, this.#catalog, this.#browserResources.resources.read(), this.#camera.read(), this.options.connected(), this.#browserResources.previewStarted, this.#sources, this.#browserResources.preparing, this.#media.profile(), this.#detail.status());
    }
}
