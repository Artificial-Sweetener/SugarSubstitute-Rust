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
import { CanvasCameraController } from '../application/CanvasCameraController.js';
import { canvasDisplayPlan } from '../application/CanvasDisplayPlan.js';
import { CanvasImageBudget } from '../application/CanvasImageBudget.js';
import { CanvasImageDecodeScheduler } from '../application/CanvasImageDecodeScheduler.js';
import { CanvasImageResources } from '../application/CanvasImageResources.js';
import { CanvasStatusPublisher } from '../application/CanvasStatusPublisher.js';
import { canvasCameraGeometry } from '../domain/CanvasCamera.js';
import { sameCanvasNavigation } from '../domain/CanvasViewState.js';
import { BrowserCanvasImages } from '../presentation/browser/BrowserCanvasImages.js';
import { BrowserCanvasInput } from '../presentation/browser/BrowserCanvasInput.js';
import { BrowserCanvasOverview } from '../presentation/browser/BrowserCanvasOverview.js';
import { browserCanvasSnapshot } from '../presentation/browser/BrowserCanvasSnapshot.js';
import { mountCanvasOverlay, } from '../presentation/vue/CanvasOverlayRuntime.js';
import { BrowserCanvasGeometry } from './BrowserCanvasGeometry.js';
import { BrowserCanvasPort } from './BrowserCanvasPort.js';
import { BrowserCanvasResources } from './BrowserCanvasResources.js';
import { applyBrowserCanvasCommand } from './BrowserCanvasActions.js';
import { BrowserCanvasSourceNavigation } from './BrowserCanvasSourceNavigation.js';
import { ensureBrowserCanvasStyles } from './BrowserCanvasStyles.js';
/** Keep composition thin: navigation, cameras, admissions, input, and DOM each retain their own owner. */
export class BrowserCanvasAttachment {
    element;
    options;
    viewport;
    port;
    #images;
    #overview;
    #input;
    #geometry;
    #camera;
    #browserResources;
    #status;
    #overlay;
    #stopView;
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
        this.#plan = this.#displayPlan();
        this.#enroll();
        const document = element.ownerDocument;
        ensureBrowserCanvasStyles(document);
        this.viewport = document.createElement('div');
        this.viewport.className = 'sc-output-viewport';
        const overlay = document.createElement('div');
        overlay.className = 'sc-output-overlay-root';
        element.append(this.viewport, overlay);
        this.#images = new BrowserCanvasImages(this.viewport, options.frames);
        this.#overview = new BrowserCanvasOverview(element, options.frames, (cells) => this.#browserResources.overview(cells), (cell) => {
            if (!this.#disposed && options.accepts())
                options.selection.apply(options.address.viewId, this.#catalog, cell.preview
                    ? { type: 'select-preview', sourceId: cell.selection.sourceId }
                    : { type: 'select', selection: cell.selection });
        });
        const budget = new CanvasImageBudget();
        const decodes = new CanvasImageDecodeScheduler(options.loader, options.clock);
        const resources = new CanvasImageResources({
            clock: options.clock,
            budget,
            decodes,
            changed: () => this.#resourcesChanged(),
            logger: options.logger,
        });
        this.#browserResources = new BrowserCanvasResources(resources, budget, options.allowances, options.url);
        this.#status = new CanvasStatusPublisher(options.clock, () => this.#publish());
        this.#camera = new CanvasCameraController({
            view: options.address.viewId,
            inspections: options.inspections,
            clock: options.frames,
            reducedMotion: () => Boolean(document.defaultView?.matchMedia?.('(prefers-reduced-motion: reduce)').matches),
            present: (target) => {
                this.#images.camera(target);
                if (this.#ready)
                    this.#status.changed();
            },
        });
        const sources = new BrowserCanvasSourceNavigation({
            resources: this.#browserResources,
            selection: options.selection,
            read: () => !this.#disposed && options.accepts() ? { plan: this.#plan, catalog: this.#catalog } : null,
            cells: (catalog) => this.#overview.cells(catalog),
        });
        this.port = new BrowserCanvasPort(this.#snapshot(), {
            origin: document.location.origin,
            capabilities: {
                schema: 1,
                commands: [
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
                ],
            },
            accepts: () => !this.#disposed && options.accepts(),
            apply: (command) => applyBrowserCanvasCommand(command, {
                plan: this.#plan,
                catalog: this.#catalog,
                camera: this.#camera,
                resources: this.#browserResources,
                selection: options.selection,
                sources,
                snapshot: command.type === 'comparison' ? this.#snapshot() : this.port.read(),
                open: options.open,
            }),
            geometry: (geometry) => this.#resize(geometry),
            release: () => this.dispose(),
            report: options.report,
        });
        const cleanup = [
            () => this.#images.dispose(),
            () => this.#overview.dispose(),
            () => this.#browserResources.dispose(),
            () => this.#status.dispose(),
            () => this.#camera.dispose(),
            () => this.port.dispose(),
        ];
        try {
            this.#overlay = mountCanvasOverlay(overlay, this.port, options.report, element);
            cleanup.push(() => this.#overlay.dispose());
            this.#input = new BrowserCanvasInput(this.viewport, {
                contains: (point) => this.#images.contains(point),
                interrupt: () => this.#camera.interrupt(),
                pan: (delta) => {
                    this.#camera.pan(delta);
                },
                zoom: (factor) => {
                    this.#camera.zoom(factor);
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
                fit: () => {
                    this.#camera.fit();
                },
            });
            cleanup.push(() => this.#input.dispose());
            this.#geometry = new BrowserCanvasGeometry(this.viewport, options.frames, (geometry) => this.port.updateSurface(options.address, geometry));
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
    /** Refresh light catalog state at a scoped execution/metadata/navigation boundary. */
    refresh() {
        if (!this.#ready || this.#disposed || !this.options.accepts())
            return;
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
        this.#plan = this.#displayPlan();
        this.#enroll();
        const linked = this.#plan.view.linked;
        if (linked !== this.#linked) {
            this.#linked = linked;
            this.#camera.link(linked);
        }
        this.#images.wipe(this.#plan.view.comparison);
        this.#camera.comparison(Boolean(this.#plan.view.comparison));
        const overview = this.#plan.view.mode === 'overview';
        this.element.classList.toggle('sc-output-overview-mode', overview);
        this.#browserResources.reconcile(this.#plan);
        this.#overview.reconcile({ ...catalog, entries: this.#plan.grid }, overview);
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
        if (this.wants(preview.sessionId, preview.generation, preview.sourceId))
            this.#browserResources.receive(preview, blob);
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
        this.#input.dispose();
        this.#geometry.dispose();
        this.#camera.dispose();
        this.#overlay.dispose();
        this.#images.dispose();
        this.#overview.dispose();
        this.#browserResources.dispose();
        this.options.disposeLoader();
        this.options.views.releaseAttachment(this.options.address);
        this.element.replaceChildren();
        this.element.classList.remove('sc-output-overview-mode');
    }
    /** Resolve current route selections through the authoritative navigation owner. */
    #displayPlan() {
        const { address, selection, views } = this.options;
        return canvasDisplayPlan(views.read(address.viewId), this.#catalog, selection.items(address.viewId, this.#catalog), selection.preview(address.viewId, this.#catalog));
    }
    /** Prune independent cameras against current output slots. */
    #enroll() {
        const slots = this.#catalog.entries.flatMap((entry) => entry.items.map((item) => ({ sourceId: entry.source.id, batchIndex: item.batchIndex })));
        for (const slot of [this.#plan.primary, this.#plan.secondary])
            if (slot?.sourcePresent)
                slots.push(slot.selection);
        this.options.inspections.synchronize(this.options.address.viewId, this.#catalog.generation, slots);
    }
    /** Reconcile an actual decoded route, including the outgoing image during pending replacement. */
    #resourcesChanged() {
        if (!this.#ready || this.#disposed)
            return;
        this.#browserResources.measured();
        const images = this.#browserResources.resources.read();
        this.#overview.images(images);
        if (this.#plan.view.mode === 'overview') {
            this.#images.images(null, null);
            this.#input.cancel();
            this.#camera.suspend();
            this.#status.changed();
            return;
        }
        const primary = images.find((slot) => slot.slot === 'primary')?.frame ?? null;
        const secondary = images.find((slot) => slot.slot === 'secondary')?.frame ?? null;
        this.#images.images(primary, secondary);
        const basis = primary ?? secondary;
        const surface = this.#surface;
        const geometry = surface?.visible && basis
            ? canvasCameraGeometry(basis.dimensions, surface.viewport, surface.physicalPixelsPerCssPixel)
            : null;
        if (basis && geometry)
            this.#camera.show(basis.selection, geometry, basis.runOrder, basis.previewLane);
        else {
            this.#input.cancel();
            this.#camera.suspend();
        }
        this.#status.changed();
    }
    /** Apply settled geometry while suspending all image admissions for zero-sized/hidden allocations. */
    #resize(surface) {
        this.#surface = surface;
        this.#browserResources.active(surface.visible);
        this.#overview.resize();
        this.#resourcesChanged();
    }
    /** Publish the current actual display at bounded cadence, without graph access. */
    #publish() {
        if (this.#ready && !this.#disposed)
            this.port.publish(this.#snapshot());
    }
    /** Report actual camera/fidelity rather than the incoming selection's unshown dimensions. */
    #snapshot() {
        return browserCanvasSnapshot(this.options.address, this.#plan, this.#catalog, this.#browserResources.resources.read(), this.#camera.read(), this.options.connected(), this.#browserResources.previewStarted, this.#sources, this.#browserResources.preparing);
    }
}
