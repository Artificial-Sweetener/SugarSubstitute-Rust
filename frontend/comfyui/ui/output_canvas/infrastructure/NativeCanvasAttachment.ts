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

import type {
  CanvasSurfaceHost,
  CanvasSurfaceViewportState,
} from '../../../../../integrations/contracts/CanvasSurface.js';
import type { WorkflowPreview } from '../../execution/domain/WorkflowPreview.js';
import { applyCanvasAction } from '../application/CanvasActions.js';
import type { CanvasDisplayPlan } from '../application/CanvasDisplayPlan.js';
import { CanvasSourceNavigation } from '../application/CanvasSourceNavigation.js';
import { CanvasStatusPublisher } from '../application/CanvasStatusPublisher.js';
import { CanvasViewProjection } from '../application/CanvasViewProjection.js';
import { canvasViewSnapshot } from '../application/CanvasViewSnapshot.js';
import type { CanvasCatalogSnapshot } from '../domain/CanvasCatalog.js';
import type { CanvasRevealTarget } from '../domain/CanvasReveal.js';
import type {
  CanvasResourceStatus,
  CanvasSourceSummary,
  CanvasSurfaceGeometry,
} from '../domain/CanvasViewPort.js';
import {
  mountCanvasOverlay,
  type MountedCanvasOverlay,
} from '../presentation/vue/CanvasOverlayRuntime.js';
import type { CanvasAttachmentOptions } from './CanvasAttachmentOptions.js';
import { CanvasMediaActions } from './CanvasMediaActions.js';
import { NativeCanvasReadiness } from './NativeCanvasReadiness.js';
import { CanvasViewportKeyboard } from '../presentation/CanvasViewportKeyboard.js';
import { CanvasSurfaceGeometryObserver } from './CanvasSurfaceGeometryObserver.js';
import { CanvasSurfacePort } from './CanvasSurfacePort.js';
import { ensureCanvasViewportStyles } from './CanvasViewportStyles.js';
import { NativeCanvasActionSurface } from './NativeCanvasActionSurface.js';
import { nativeCanvasViewportCommands } from './NativeCanvasCapabilities.js';
import {
  NativeCanvasPresentation,
  type NativeCanvasPresentationStatus,
} from './NativeCanvasPresentation.js';

/** Supply the negotiated host through the existing canvas capability registry. */
export interface NativeCanvasAttachmentOptions extends CanvasAttachmentOptions {
  nativeSurface(): CanvasSurfaceHost | null;
  /** Check the staged registry lease without granting active control or preview authority. */
  acceptsPreparation(): boolean;
}

/** Retain controls and selection while leaving every output pixel to the native host. */
export class NativeCanvasAttachment {
  readonly ready: Promise<void>;
  readonly #readiness: NativeCanvasReadiness;
  readonly viewport: HTMLDivElement;
  readonly port: CanvasSurfacePort;
  readonly #projection: CanvasViewProjection;
  readonly #status: CanvasStatusPublisher;
  readonly #media: CanvasMediaActions;
  readonly #native: NativeCanvasPresentation;
  readonly #geometry: CanvasSurfaceGeometryObserver;
  readonly #overlay: MountedCanvasOverlay;
  readonly #stopView: () => void;
  readonly #stopTransfer: () => void;
  readonly #keyboard: CanvasViewportKeyboard;
  #catalog: CanvasCatalogSnapshot;
  #sources: readonly CanvasSourceSummary[];
  #plan: CanvasDisplayPlan;
  #surface: CanvasSurfaceGeometry | null = null;
  #viewport: CanvasSurfaceViewportState | null = null;
  #presentation: NativeCanvasPresentationStatus = { state: 'empty', message: '' };
  #ready = false;
  #disposed = false;

  /** Mount shared controls without constructing browser loaders, image layers, or decoders. */
  constructor(
    private readonly element: HTMLElement,
    private readonly options: NativeCanvasAttachmentOptions,
  ) {
    const catalog = options.catalog();
    if (!catalog || catalog.sessionId !== options.address.sessionId)
      throw new Error('Canvas workflow catalog is unavailable.');
    this.#catalog = catalog;
    this.#sources = Object.freeze(catalog.entries.map((entry) => entry.summary));
    this.#projection = new CanvasViewProjection(
      options.address.viewId,
      options.views,
      options.selection,
      options.inspections,
    );
    this.#plan = this.#projection.project(catalog);
    const document = element.ownerDocument;
    ensureCanvasViewportStyles(document);
    this.viewport = document.createElement('div');
    this.viewport.className = 'sc-output-viewport';
    this.viewport.setAttribute('data-sc-canvas-geometry', '');
    const overlay = document.createElement('div');
    overlay.className = 'sc-output-overlay-root';
    element.append(this.viewport, overlay);
    this.#status = new CanvasStatusPublisher(options.clock, () => this.#publish());
    this.#media = new CanvasMediaActions({
      read: () => this.#snapshot(),
      accepts: () => !this.#disposed && options.accepts(),
      ...(options.copy ? { copy: options.copy } : {}),
      ...(options.fileAction ? { fileAction: options.fileAction } : {}),
      ...(options.mediaProfile ? { profile: options.mediaProfile } : {}),
    });
    const sources = new CanvasSourceNavigation({
      resources: { prepareSource: () => null, cancelPreparation: () => {} },
      selection: options.selection,
      read: () =>
        !this.#disposed && options.accepts() ? { plan: this.#plan, catalog: this.#catalog } : null,
      cells: () => [],
    });
    const surface = new NativeCanvasActionSurface(this.#media, options.open, (command, scope) =>
      this.#native.viewport(command, scope),
    );
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
          ...nativeCanvasViewportCommands(options.nativeSurface()),
          ...this.#media.commands(),
        ],
      },
      accepts: () => !this.#disposed && options.accepts(),
      apply: (command, scope) =>
        applyCanvasAction(command, {
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
      acceptsPreparation: () => !this.#disposed && options.acceptsPreparation(),
      prepared: () => this.#readiness.complete(),
      host: options.nativeSurface,
      accepts: () => !this.#disposed && options.accepts(),
      visible: (value) => this.viewport.classList.toggle('sc-output-native', value),
      status: (value) => {
        this.#presentation = value;
        if (value.state === 'error' || value.state === 'unavailable')
          this.#readiness.fail(new Error(value.message));
        if (this.#ready) this.#status.changed(true);
      },
      viewport: (value) => {
        this.#viewport = value;
        if (this.#ready) this.#status.changed(true);
      },
      report: (error) => options.report(error, null),
    });
    this.#stopTransfer =
      options.transferChanged?.(() => {
        if (this.#ready) this.#status.changed(true);
      }) ?? (() => {});
    this.#keyboard = new CanvasViewportKeyboard(this.viewport, this.port, options.report);
    this.viewport.addEventListener('pointerdown', this.#focusViewport);
    this.#readiness = new NativeCanvasReadiness(options.clock);
    this.ready = this.#readiness.ready;
    const cleanup: (() => void)[] = [
      () => this.#readiness.dispose(),
      () => this.#keyboard.dispose(),
      () => this.viewport.removeEventListener('pointerdown', this.#focusViewport),
      () => this.#native.dispose(),
      () => this.#media.dispose(),
      () => this.#status.dispose(),
      () => this.port.dispose(),
      this.#stopTransfer,
    ];
    try {
      this.#overlay = mountCanvasOverlay(overlay, this.port, options.report, element);
      cleanup.push(() => this.#overlay.dispose());
      this.#geometry = new CanvasSurfaceGeometryObserver(
        this.viewport,
        options.frames,
        (geometry) => this.#observeGeometry(geometry),
      );
      cleanup.push(() => this.#geometry.dispose());
      this.#stopView = options.views.subscribe(options.address.viewId, () => this.refresh());
      cleanup.push(this.#stopView);
      this.#ready = true;
      this.#resize(this.#geometry.read());
      this.refresh();
    } catch (error: unknown) {
      this.#ready = false;
      this.#disposed = true;
      for (const stop of cleanup.reverse()) stop();
      options.views.releaseAttachment(options.address);
      element.replaceChildren();
      throw error;
    }
  }

  /** Navigate graph-thumbnail inspection through the same shared selection authority. */
  async reveal(target: CanvasRevealTarget): Promise<boolean> {
    if (this.#disposed || !this.options.accepts()) return false;
    const request = this.port.createRequest({ type: 'reveal', target }, this.port.read().revision);
    return (await this.port.dispatch(request)).status === 'applied';
  }

  /** Reconcile the current workflow and its selected native output. */
  refresh(): void {
    if (
      !this.#ready ||
      this.#disposed ||
      !(this.options.accepts() || this.options.acceptsPreparation())
    )
      return;
    if (!this.#surface?.visible) this.#observeGeometry(this.#geometry.read());
    const catalog = this.options.catalog();
    if (!catalog || catalog.generation !== this.#catalog.generation) return;
    this.#catalog = catalog;
    this.#sources = Object.freeze(catalog.entries.map((entry) => entry.summary));
    this.#plan = this.#projection.project(catalog);
    this.#native.update(this.#surface, this.#plan.primary?.item ?? null);
    this.#status.changed(true);
  }

  /** Refuse preview bytes until the native host exposes preview presentation. */
  wants(): boolean {
    return false;
  }

  /** Preview delivery is disabled by the admission decision above. */
  receive(_preview: WorkflowPreview, _blob: Blob): void {}

  /** Report zero browser image allocations for this attachment. */
  diagnostics() {
    return {
      displayed: 0,
      pendingDecodes: 0,
      queuedInputs: 0,
      previewDrops: 0,
      estimatedBytes: 0,
      unknownLoads: 0,
      activeDeadlines: 0,
      renderFrames: 0,
    };
  }

  /** Release the native surface and shared controls before retiring their view lease. */
  dispose(): Promise<void> {
    if (this.#disposed) return this.#native.dispose();
    this.#disposed = true;
    this.#ready = false;
    this.#readiness.dispose();
    this.#keyboard.dispose();
    this.viewport.removeEventListener('pointerdown', this.#focusViewport);
    this.#stopView();
    this.#stopTransfer();
    this.#status.dispose();
    this.port.dispose();
    this.#geometry.dispose();
    this.#overlay.dispose();
    this.#media.dispose();
    const released = this.#native.dispose();
    this.options.views.releaseAttachment(this.options.address);
    this.element.replaceChildren();
    return released;
  }

  /** Give discrete viewer keys focus without capturing or forwarding native pointer gestures. */
  readonly #focusViewport = (event: PointerEvent): void => {
    if (event.button === 0 && event.target === this.viewport && this.options.accepts())
      this.viewport.focus({ preventScroll: true });
  };

  /** Measure hidden candidates without granting them committed control-port authority. */
  #observeGeometry(surface: CanvasSurfaceGeometry): void {
    if (this.options.acceptsPreparation()) this.#resize(surface);
    else this.port.updateSurface(this.options.address, surface);
  }

  /** Forward qualified geometry without constructing a browser drawing surface. */
  #resize(surface: CanvasSurfaceGeometry): void {
    this.#surface = surface;
    this.#native.update(surface, this.#plan.primary?.item ?? null);
  }

  /** Publish selection and actual native readiness to the shared overlay. */
  #publish(): void {
    if (!this.#ready || this.#disposed) return;
    this.#media.reconcile();
    this.port.publish(this.#snapshot());
  }

  /** Describe only capabilities and state supported by the current native surface. */
  #snapshot() {
    const item = this.#plan.primary?.item;
    let resource: CanvasResourceStatus = {
      ...this.#presentation,
      canRetry: false,
      canAdmitLargeImage: false,
    };
    if (item && item.mediaKind !== 'image') {
      resource = {
        ...resource,
        state: 'unavailable',
        message: 'Open this media in the host viewer.',
      };
    } else if (!item && this.#plan.preview) {
      resource = {
        ...resource,
        state: 'unavailable',
        message: 'Live previews are not available on this surface.',
      };
    } else if (this.#plan.primary && !item) {
      resource = {
        ...resource,
        state: 'unavailable',
        message: 'The selected workflow output is unavailable.',
      };
    }
    const snapshot = canvasViewSnapshot(
      this.options.address,
      this.#plan,
      this.#catalog,
      this.#sources,
      this.options.connected(),
      this.#media.profile(),
      {
        comparisonAvailable: false,
        preview: false,
        resource,
        secondaryResource: null,
        zoom: this.#viewport
          ? { ...this.#viewport, secondaryPhysicalScale: null, secondaryDimensions: null }
          : null,
      },
    );
    return Object.freeze({
      ...snapshot,
      secondary: null,
      grid: Object.freeze([]),
      view: Object.freeze({ ...snapshot.view, mode: 'image' as const, comparison: null }),
    });
  }
}
