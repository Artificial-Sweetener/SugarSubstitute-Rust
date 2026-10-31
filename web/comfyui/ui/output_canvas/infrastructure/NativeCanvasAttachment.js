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
/** Compose shared canvas controls over one host-owned native output surface. */
import { applyCanvasAction } from '../application/CanvasActions.js';
import { CanvasSourceNavigation } from '../application/CanvasSourceNavigation.js';
import { CanvasStatusPublisher } from '../application/CanvasStatusPublisher.js';
import { CanvasViewProjection } from '../application/CanvasViewProjection.js';
import { mountCanvasOverlay, } from '../presentation/vue/CanvasOverlayRuntime.js';
import { CanvasMediaActions } from './CanvasMediaActions.js';
import { NativeCanvasReadiness } from './NativeCanvasReadiness.js';
import { CanvasViewportKeyboard } from '../presentation/CanvasViewportKeyboard.js';
import { CanvasSurfaceGeometryObserver } from './CanvasSurfaceGeometryObserver.js';
import { CanvasSurfacePort } from './CanvasSurfacePort.js';
import { ensureCanvasViewportStyles } from './CanvasViewportStyles.js';
import { NativeCanvasActionSurface } from './NativeCanvasActionSurface.js';
import { nativeCanvasCommands, nativeCanvasComparison, nativeCanvasInput, nativeCanvasOverview, } from './NativeCanvasCapabilities.js';
import { NativeCanvasOcclusions } from './NativeCanvasOcclusions.js';
import { NativeCanvasPendingReveal } from './NativeCanvasPendingReveal.js';
import { NativeCanvasOverview } from './NativeCanvasOverview.js';
import { nativeCanvasSnapshot } from './NativeCanvasSnapshot.js';
import { NativeCanvasPresentation, } from './NativeCanvasPresentation.js';
/** Retain controls and selection while leaving every output pixel to the native host. */
export class NativeCanvasAttachment {
    element;
    options;
    ready;
    #readiness;
    viewport;
    port;
    #projection;
    #status;
    #media;
    #native;
    #geometry;
    #overlay;
    #stopView;
    #stopTransfer;
    #keyboard;
    /** Fixed per attachment; changed host capabilities rebuild the attachment. */
    #comparison;
    /** Present only for a host that paints overview cells; fixed like the comparison flag. */
    #overview;
    #reveal;
    #catalog;
    #sources;
    #plan;
    #surface = null;
    #viewport = null;
    #preview = false;
    #presentation = { state: 'empty', message: '' };
    #ready = false;
    #disposed = false;
    /** Mount shared controls without constructing browser loaders, image layers, or decoders. */
    constructor(element, options) {
        this.element = element;
        this.options = options;
        const catalog = options.catalog();
        if (!catalog || catalog.sessionId !== options.address.sessionId)
            throw new Error('Canvas workflow catalog is unavailable.');
        this.#catalog = catalog;
        this.#sources = Object.freeze(catalog.entries.map((entry) => entry.summary));
        this.#projection = new CanvasViewProjection(options.address.viewId, options.views, options.selection, options.inspections);
        this.#plan = this.#projection.project(catalog);
        this.#comparison = nativeCanvasComparison(options.nativeSurface());
        const document = element.ownerDocument;
        ensureCanvasViewportStyles(document);
        this.viewport = document.createElement('div');
        this.viewport.className = 'sc-output-viewport';
        this.viewport.setAttribute('data-sc-canvas-geometry', '');
        const overlay = document.createElement('div');
        overlay.className = 'sc-output-overlay-root';
        element.append(this.viewport, overlay);
        const host = options.nativeSurface();
        this.viewport.classList.toggle('sc-output-native-input', nativeCanvasInput(host));
        this.#overview =
            host && nativeCanvasOverview(host)
                ? new NativeCanvasOverview(element, {
                    frames: options.frames,
                    address: options.address,
                    capabilities: host.capabilities,
                    geometry: () => this.#surface,
                    select: (cell) => {
                        if (!this.#disposed && options.accepts())
                            options.selection.apply(options.address.viewId, this.#catalog, {
                                type: 'select',
                                selection: cell.selection,
                            });
                    },
                    changed: () => {
                        if (this.#ready)
                            this.#present();
                    },
                })
                : null;
        this.#status = new CanvasStatusPublisher(options.clock, () => this.#publish());
        this.#media = new CanvasMediaActions({
            read: () => this.#snapshot(),
            accepts: () => !this.#disposed && options.accepts(),
            ...(options.copy ? { copy: options.copy } : {}),
            ...(options.fileAction ? { fileAction: options.fileAction } : {}),
            ...(options.mediaProfile ? { profile: options.mediaProfile } : {}),
        });
        const sources = new CanvasSourceNavigation({
            resources: { prepareSource: () => null, cancelPreparation: () => { } },
            selection: options.selection,
            read: () => !this.#disposed && options.accepts() ? { plan: this.#plan, catalog: this.#catalog } : null,
            cells: () => [],
        });
        const surface = new NativeCanvasActionSurface(this.#media, options.open, (command, scope) => this.#native.viewport(command, scope));
        this.port = new CanvasSurfacePort(this.#snapshot(), {
            origin: document.location.origin,
            capabilities: {
                schema: 1,
                commands: [
                    'reveal',
                    'select',
                    'select-source',
                    'batch',
                    'follow',
                    'open-media',
                    ...nativeCanvasCommands(options.nativeSurface()),
                    ...this.#media.commands(),
                ],
            },
            accepts: () => !this.#disposed && options.accepts(),
            apply: (command, scope) => applyCanvasAction(command, {
                plan: this.#plan,
                catalog: this.#catalog,
                selection: options.selection,
                sources,
                snapshot: this.port.read(),
                surface,
                scope,
            }),
            geometry: (geometry) => this.#resize(geometry),
            release: () => this.dispose(),
            report: options.report,
        });
        this.#native = new NativeCanvasPresentation({
            address: options.address,
            clock: options.clock,
            acceptsPreparation: () => !this.#disposed && options.acceptsPreparation(),
            prepared: () => this.#readiness.complete(),
            host: options.nativeSurface,
            surfacePending: () => this.options.surfacePending?.() ?? false,
            accepts: () => !this.#disposed && options.accepts(),
            visible: (value) => this.viewport.classList.toggle('sc-output-native', value),
            status: (value) => {
                this.#presentation = value;
                if (value.state === 'error' || value.state === 'unavailable')
                    this.#readiness.fail(new Error(value.message));
                if (this.#ready)
                    this.#status.changed(true);
            },
            viewport: (value) => {
                this.#viewport = value;
                if (this.#ready)
                    this.#status.changed(true);
            },
            preview: (value) => {
                this.#preview = value;
                if (this.#ready)
                    this.#status.changed(true);
            },
            report: (error) => options.report(error, null),
        });
        this.#stopTransfer =
            options.transferChanged?.(() => {
                if (this.#ready)
                    this.#status.changed(true);
            }) ?? (() => { });
        this.#reveal = new NativeCanvasPendingReveal({
            dispatch: (target) => this.#dispatchReveal(target),
            revision: () => options.views.read(options.address.viewId).revision,
            report: (error) => options.report(error, null),
        });
        this.#keyboard = new CanvasViewportKeyboard(this.viewport, this.port, options.report);
        this.viewport.addEventListener('pointerdown', this.#focusViewport);
        this.#readiness = new NativeCanvasReadiness(options.clock);
        this.ready = this.#readiness.ready;
        const cleanup = [
            () => this.#readiness.dispose(),
            () => this.#keyboard.dispose(),
            () => this.#reveal.drop(),
            () => this.viewport.removeEventListener('pointerdown', this.#focusViewport),
            () => this.#native.dispose(),
            () => this.#overview?.dispose(),
            () => this.#media.dispose(),
            () => this.#status.dispose(),
            () => this.port.dispose(),
            this.#stopTransfer,
        ];
        try {
            this.#overlay = mountCanvasOverlay(overlay, this.port, options.report, element);
            cleanup.push(() => this.#overlay.dispose());
            this.#geometry = new CanvasSurfaceGeometryObserver(this.viewport, options.frames, (geometry) => this.#observeGeometry(geometry), 
            // The host routes OS gestures itself, around the web content the page reports covering it.
            nativeCanvasInput(host) ? new NativeCanvasOcclusions(element, this.viewport) : null);
            cleanup.push(() => this.#geometry.dispose());
            this.#stopView = options.views.subscribe(options.address.viewId, () => this.refresh());
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
            options.views.releaseAttachment(options.address);
            element.replaceChildren();
            throw error;
        }
    }
    /**
     * Navigate graph-thumbnail inspection through the same shared selection authority.
     *
     * A staged candidate cannot accept commands until it commits, so it retains the latest reveal
     * and applies it once it accepts, like a browser attachment awaiting source preparation.
     */
    async reveal(target) {
        if (this.#disposed)
            return false;
        if (this.options.accepts())
            return await this.#dispatchReveal(target);
        if (!this.options.acceptsPreparation())
            return false;
        return await this.#reveal.retain(target);
    }
    /** Reconcile the current workflow and its selected native output. */
    refresh() {
        if (!this.#ready ||
            this.#disposed ||
            !(this.options.accepts() || this.options.acceptsPreparation()))
            return;
        if (!this.#surface?.visible)
            this.#observeGeometry(this.#geometry.read());
        const catalog = this.options.catalog();
        if (!catalog || catalog.generation !== this.#catalog.generation)
            return;
        this.#catalog = catalog;
        this.#sources = Object.freeze(catalog.entries.map((entry) => entry.summary));
        this.#plan = this.#projection.project(catalog);
        const overview = this.#overview !== null && this.#plan.view.mode === 'overview';
        this.element.classList.toggle('sc-output-overview-mode', overview);
        this.#overview?.reconcile(catalog, this.#plan.view, overview);
        this.#present();
        this.#status.changed(true);
        if (this.options.accepts())
            this.#reveal.apply();
    }
    /** Admit bytes only for this visible, committed view's authoritative source/run interest. */
    wants(session, generation, source) {
        return (!this.#disposed &&
            this.#surface?.visible === true &&
            this.options.accepts() &&
            this.#catalog.result.context.sessionId === session &&
            this.#catalog.generation === generation &&
            this.options.selection.allowsSourcePreview(this.options.address.viewId, this.#catalog, source) &&
            this.#native.wants());
    }
    /** Preserve original token/Blob custody while native transfer remains independent of browser decoding. */
    receive(preview, blob) {
        if (this.wants(preview.sessionId, preview.generation, preview.sourceId))
            this.#native.receive(preview, blob);
    }
    /** Report zero browser image allocations for this attachment. */
    diagnostics() {
        return {
            displayed: 0,
            pendingDecodes: 0,
            unknownLoads: 0,
            renderFrames: 0,
            ...this.#native.diagnostics(),
        };
    }
    /** Release the native surface and shared controls before retiring their view lease. */
    dispose() {
        if (this.#disposed)
            return this.#native.dispose();
        this.#disposed = true;
        this.#ready = false;
        this.#readiness.dispose();
        this.#keyboard.dispose();
        this.#reveal.drop();
        this.viewport.removeEventListener('pointerdown', this.#focusViewport);
        this.#stopView();
        this.#stopTransfer();
        this.#status.dispose();
        this.port.dispose();
        this.#geometry.dispose();
        this.#overlay.dispose();
        this.#media.dispose();
        const released = this.#native.dispose();
        this.#overview?.dispose();
        this.options.views.releaseAttachment(this.options.address);
        this.element.replaceChildren();
        this.element.classList.remove('sc-output-overview-mode');
        return released;
    }
    /** Dispatch a reveal through the shared command port, which requires committed acceptance. */
    async #dispatchReveal(target) {
        const request = this.port.createRequest({ type: 'reveal', target }, this.port.read().revision);
        return (await this.port.dispatch(request)).status === 'applied';
    }
    /** Give discrete viewer keys focus; the host routes native pointer gestures itself. */
    #focusViewport = (event) => {
        if (event.button === 0 && event.target === this.viewport && this.options.accepts())
            this.viewport.focus({ preventScroll: true });
    };
    /** Measure hidden candidates without granting them committed control-port authority. */
    #observeGeometry(surface) {
        if (this.options.acceptsPreparation())
            this.#resize(surface);
        else
            this.port.updateSurface(this.options.address, surface);
    }
    /** Forward qualified geometry without constructing a browser drawing surface. */
    #resize(surface) {
        // Unchanged measurements (e.g. a remeasure after the attachment's own render) are idle.
        if (this.#surface && JSON.stringify(this.#surface) === JSON.stringify(surface))
            return;
        this.#surface = surface;
        this.#overview?.resize();
        this.#present();
    }
    /**
     * Hand the current plan's A, optional B and divider, or its overview cells in their place, to
     * the native presentation owner. Live previews do not stream while the overview is shown.
     */
    #present() {
        if (this.#overview && this.#plan.view.mode === 'overview') {
            this.#native.update(this.#surface, null, null, null, this.#overview.cells);
            return;
        }
        const comparison = this.#comparison ? this.#plan.view.comparison : null;
        const secondary = this.#plan.secondary?.item;
        const pair = comparison && secondary
            ? { item: secondary, orientation: comparison.orientation, split: comparison.split }
            : null;
        this.#native.update(this.#surface, this.#plan.primary?.item ?? null, this.#plan.preview, pair);
    }
    /** Publish selection and actual native readiness to the shared overlay. */
    #publish() {
        if (!this.#ready || this.#disposed)
            return;
        this.#media.reconcile();
        this.port.publish(this.#snapshot());
    }
    /** Describe only capabilities and state supported by the current native surface. */
    #snapshot() {
        return nativeCanvasSnapshot({
            address: this.options.address,
            plan: this.#plan,
            catalog: this.#catalog,
            sources: this.#sources,
            connected: this.options.connected(),
            mediaActions: this.#media.profile(),
            presentation: this.#presentation,
            viewport: this.#viewport,
            preview: this.#preview,
            // Shared controls read a first snapshot before the native presentation owner exists.
            previewAvailable: this.#native?.previewAvailable ?? false,
            comparison: this.#comparison,
            overview: this.#overview !== null,
        });
    }
}
