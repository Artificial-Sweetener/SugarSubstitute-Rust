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

import type { WorkflowOutputItem } from '../../execution/domain/WorkflowOutput.js';
import type { ExecutionClock } from '../../execution/domain/WorkflowExecutionEvent.js';
import type { WorkflowPreview } from '../../execution/domain/WorkflowPreview.js';
import type {
  WorkflowSessionId,
  OutputSourceId,
} from '../../execution/domain/ExecutionIdentity.js';
import { CanvasCameraController } from '../application/CanvasCameraController.js';
import type { CanvasFrameClock } from '../application/CanvasCameraTransition.js';
import { canvasDisplayPlan, type CanvasDisplayPlan } from '../application/CanvasDisplayPlan.js';
import { CanvasImageBudget } from '../application/CanvasImageBudget.js';
import type { CanvasImageAllowances } from '../application/CanvasImageAllowances.js';
import { CanvasImageDecodeScheduler } from '../application/CanvasImageDecodeScheduler.js';
import { CanvasImageResources } from '../application/CanvasImageResources.js';
import type { CanvasInspectionStore } from '../application/CanvasInspectionStore.js';
import type { CanvasSelectionController } from '../application/CanvasSelectionController.js';
import { CanvasStatusPublisher } from '../application/CanvasStatusPublisher.js';
import type { CanvasViewRegistry } from '../application/CanvasViewRegistry.js';
import { canvasCameraGeometry } from '../domain/CanvasCamera.js';
import type { CanvasCatalogSnapshot } from '../domain/CanvasCatalog.js';
import type { CanvasAttachmentAddress } from '../domain/CanvasIdentity.js';
import type { CanvasImageLoadPort } from '../domain/CanvasImageResource.js';
import type { CanvasCommandRequest, CanvasSurfaceGeometry } from '../domain/CanvasViewPort.js';
import type { CanvasSourceSummary } from '../domain/CanvasViewPort.js';
import { sameCanvasNavigation } from '../domain/CanvasViewState.js';
import { BrowserCanvasImages } from '../presentation/browser/BrowserCanvasImages.js';
import { BrowserCanvasInput } from '../presentation/browser/BrowserCanvasInput.js';
import { BrowserCanvasOverview } from '../presentation/browser/BrowserCanvasOverview.js';
import { browserCanvasSnapshot } from '../presentation/browser/BrowserCanvasSnapshot.js';
import {
  mountCanvasOverlay,
  type MountedCanvasOverlay,
} from '../presentation/vue/CanvasOverlayRuntime.js';
import { BrowserCanvasGeometry } from './BrowserCanvasGeometry.js';
import { BrowserCanvasPort } from './BrowserCanvasPort.js';
import type { CanvasRevealTarget } from '../domain/CanvasReveal.js';
import { BrowserCanvasResources } from './BrowserCanvasResources.js';
import type { BrowserCanvasImagePayload } from './BrowserCanvasImageLoader.js';
import { applyBrowserCanvasCommand } from './BrowserCanvasActions.js';
import { BrowserCanvasSourceNavigation } from './BrowserCanvasSourceNavigation.js';
import { ensureBrowserCanvasStyles } from './BrowserCanvasStyles.js';
import {
  BrowserCanvasDrag,
  type BrowserCanvasDragPayload,
} from '../presentation/browser/BrowserCanvasDrag.js';
import { BrowserCanvasDragSubjects } from './BrowserCanvasDragSubject.js';
import {
  BrowserCanvasMediaActions,
  type BrowserCanvasMediaOptions,
} from './BrowserCanvasMediaActions.js';

export interface BrowserCanvasAttachmentOptions {
  readonly address: CanvasAttachmentAddress;
  readonly views: CanvasViewRegistry;
  readonly selection: CanvasSelectionController;
  readonly inspections: CanvasInspectionStore;
  readonly allowances: CanvasImageAllowances;
  readonly clock: ExecutionClock;
  readonly frames: CanvasFrameClock;
  readonly loader: CanvasImageLoadPort<BrowserCanvasImagePayload, HTMLImageElement>;
  readonly logger: Pick<Console, 'error'>;
  /** Read the authoritative active catalog only at result/navigation boundaries. */
  catalog(): CanvasCatalogSnapshot | null;
  /** Check the fixed attachment and workflow generation without graph traversal. */
  accepts(): boolean;
  /** Read shared connection state without installing another execution listener. */
  connected(): boolean;
  /** Build the host's validated full-fidelity file URL. */
  url(item: WorkflowOutputItem): string;
  /** Open supported media through the host-facing browser adapter. */
  open(item: WorkflowOutputItem): void;
  /** Copy a completed image through the platform's shared transfer adapter. */
  copy?: BrowserCanvasMediaOptions['copy'];
  fileAction?: BrowserCanvasMediaOptions['fileAction'];
  mediaProfile?: BrowserCanvasMediaOptions['profile'];
  /** Prepare one exact completed image for platform-owned outbound dragging. */
  drag?(item: WorkflowOutputItem, signal: AbortSignal): Promise<BrowserCanvasDragPayload | null>;
  /** Invalidate prepared transfers after an accepted shared preference change. */
  transferChanged?(listener: () => void): () => void;
  /** Dispose platform loading after the resource owner has canceled every lease. */
  disposeLoader(): void;
  /** Report product feedback through the existing SugarCubes services. */
  report(error: unknown, request: CanvasCommandRequest | null): void;
}

/** Keep composition thin: navigation, cameras, admissions, input, and DOM each retain their own owner. */
export class BrowserCanvasAttachment {
  readonly viewport: HTMLDivElement;
  readonly port: BrowserCanvasPort;
  readonly #images: BrowserCanvasImages;
  readonly #overview: BrowserCanvasOverview;
  readonly #input: BrowserCanvasInput;
  readonly #drag: BrowserCanvasDrag;
  readonly #media: BrowserCanvasMediaActions;
  readonly #stopTransfer: () => void;
  readonly #geometry: BrowserCanvasGeometry;
  readonly #camera: CanvasCameraController;
  readonly #browserResources: BrowserCanvasResources;
  readonly #status: CanvasStatusPublisher;
  readonly #navigation: BrowserCanvasSourceNavigation;
  readonly #overlay: MountedCanvasOverlay;
  readonly #stopView: () => void;
  #catalog: CanvasCatalogSnapshot;
  #sources: readonly CanvasSourceSummary[];
  #sourceSignature: string;
  #plan: CanvasDisplayPlan;
  #surface: CanvasSurfaceGeometry | null = null;
  #signature = '';
  #linked = true;
  #ready = false;
  #disposed = false;

  /** Mount already-qualified owners into an explicitly allocated workspace element. */
  constructor(
    private readonly element: HTMLElement,
    private readonly options: BrowserCanvasAttachmentOptions,
  ) {
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
    this.#overview = new BrowserCanvasOverview(
      element,
      options.frames,
      (cells) => this.#browserResources.overview(cells),
      (cell) => {
        if (!this.#disposed && options.accepts())
          options.selection.apply(
            options.address.viewId,
            this.#catalog,
            cell.preview
              ? { type: 'select-preview', sourceId: cell.selection.sourceId }
              : { type: 'select', selection: cell.selection },
          );
      },
    );
    const budget = new CanvasImageBudget();
    const decodes = new CanvasImageDecodeScheduler(options.loader, options.clock);
    const resources = new CanvasImageResources({
      clock: options.clock,
      budget,
      decodes,
      changed: () => this.#resourcesChanged(),
      logger: options.logger,
    });
    this.#browserResources = new BrowserCanvasResources(
      resources,
      budget,
      options.allowances,
      options.url,
    );
    const dragSubjects = new BrowserCanvasDragSubjects(
      this.viewport,
      this.#images,
      this.#overview,
      () => this.#plan,
    );
    this.#drag = new BrowserCanvasDrag(element, {
      resolve: (target, event) => dragSubjects.resolve(target, event),
      prepare: (key, signal) => {
        const item = dragSubjects.item(key);
        return item && options.drag ? options.drag(item, signal) : Promise.resolve(null);
      },
      accepts: () =>
        !this.#disposed && this.#surface?.visible === true && options.accepts() && !!options.drag,
      report: (error) => options.report(error, null),
    });
    this.#stopTransfer =
      options.transferChanged?.(() => {
        this.#drag.invalidate();
        if (this.#ready) this.#publish();
      }) ?? (() => {});
    this.#status = new CanvasStatusPublisher(options.clock, () => this.#publish());
    this.#camera = new CanvasCameraController({
      view: options.address.viewId,
      inspections: options.inspections,
      clock: options.frames,
      reducedMotion: () =>
        Boolean(document.defaultView?.matchMedia?.('(prefers-reduced-motion: reduce)').matches),
      present: (target) => {
        this.#images.camera(target);
        this.#drag.reconcile();
        if (this.#ready) this.#status.changed();
      },
    });
    const sources = new BrowserCanvasSourceNavigation({
      resources: this.#browserResources,
      selection: options.selection,
      read: () =>
        !this.#disposed && options.accepts() ? { plan: this.#plan, catalog: this.#catalog } : null,
      cells: (catalog) => this.#overview.cells(catalog),
    });
    this.#navigation = sources;
    this.#media = new BrowserCanvasMediaActions({
      read: () => this.#snapshot(),
      accepts: () => !this.#disposed && options.accepts(),
      ...(options.copy ? { copy: options.copy } : {}),
      ...(options.fileAction ? { fileAction: options.fileAction } : {}),
      ...(options.mediaProfile ? { profile: options.mediaProfile } : {}),
    });
    this.port = new BrowserCanvasPort(this.#snapshot(), {
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
      apply: (command) =>
        applyBrowserCanvasCommand(command, {
          plan: this.#plan,
          catalog: this.#catalog,
          camera: this.#camera,
          resources: this.#browserResources,
          selection: options.selection,
          sources,
          snapshot: command.type === 'comparison' ? this.#snapshot() : this.port.read(),
          open: options.open,
          media: this.#media,
        }),
      geometry: (geometry) => this.#resize(geometry),
      release: () => this.dispose(),
      report: options.report,
    });
    const cleanup: (() => void)[] = [
      () => this.#drag.dispose(),
      () => this.#media.dispose(),
      this.#stopTransfer,
      () => this.#images.dispose(),
      () => this.#overview.dispose(),
      () => this.#browserResources.dispose(),
      () => this.#status.dispose(),
      () => this.#camera.dispose(),
      () => this.port.dispose(),
    ];
    try {
      this.#overlay = mountCanvasOverlay(
        overlay,
        this.port,
        options.report,
        element,
        (target) =>
          this.#overview.contextTarget(target) ?? this.#images.contextTarget(target) ?? 'primary',
      );
      cleanup.push(() => this.#overlay.dispose());
      this.#input = new BrowserCanvasInput(this.viewport, {
        outbound: (event) => this.#drag.allows(event),
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
      this.#geometry = new BrowserCanvasGeometry(this.viewport, options.frames, (geometry) =>
        this.port.updateSurface(options.address, geometry),
      );
      cleanup.push(() => this.#geometry.dispose());
      this.#stopView = options.views.subscribe(options.address.viewId, (view) => {
        if (sameCanvasNavigation(this.#plan.view, view)) {
          const orientationChanged =
            this.#plan.view.comparison?.orientation !== view.comparison?.orientation;
          this.#plan = Object.freeze({ ...this.#plan, view });
          this.#images.wipe(view.comparison);
          this.#status.changed(orientationChanged);
        } else this.refresh();
      });
      cleanup.push(this.#stopView);
      this.#ready = true;
      this.#resize(this.#geometry.read());
      this.refresh();
    } catch (error: unknown) {
      this.#ready = false;
      this.#disposed = true;
      for (const stop of cleanup.reverse()) stop();
      options.disposeLoader();
      options.views.releaseAttachment(options.address);
      element.replaceChildren();
      throw error;
    }
  }

  /** Accept graph-thumbnail inspection through the same preparation path as surface commands. */
  async reveal(target: CanvasRevealTarget): Promise<boolean> {
    if (this.#disposed || !this.options.accepts()) return false;
    return await this.#navigation.reveal(target);
  }

  /** Refresh light catalog state at a scoped execution/metadata/navigation boundary. */
  refresh(): void {
    if (!this.#ready || this.#disposed || !this.options.accepts()) return;
    const catalog = this.options.catalog();
    if (!catalog || catalog.generation !== this.#catalog.generation) return;
    this.#catalog = catalog;
    const sources = catalog.entries.map((entry) => entry.summary);
    const sourceSignature = JSON.stringify(sources);
    if (sourceSignature !== this.#sourceSignature) {
      this.#sources = Object.freeze(sources);
      this.#sourceSignature = sourceSignature;
    }
    this.#plan = this.#displayPlan();
    this.#drag.reconcile();
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
    } else this.#status.changed();
  }

  /** Admit only bytes qualified to this visible attachment's current source and workflow. */
  receive(preview: WorkflowPreview, blob: Blob): void {
    if (this.wants(preview.sessionId, preview.generation, preview.sourceId))
      this.#browserResources.receive(preview, blob);
  }

  /** Refuse byte delivery while hidden, retired, or inspecting an exact batch/comparison. */
  wants(session: WorkflowSessionId, generation: number, source: OutputSourceId): boolean {
    return (
      !this.#disposed &&
      this.#surface?.visible === true &&
      this.options.accepts() &&
      this.#catalog.result.context.sessionId === session &&
      this.#catalog.generation === generation &&
      this.options.selection.allowsSourcePreview(this.options.address.viewId, this.#catalog, source)
    );
  }

  /** Count this attachment's owned resources and frames at explicit diagnostic boundaries. */
  diagnostics() {
    return {
      ...this.#browserResources.resources.diagnostics(),
      renderFrames: this.#images.pendingFrames,
    };
  }

  /** Retire local presentation before canceling platform images and releasing the attachment lease. */
  dispose(): void {
    if (this.#disposed) return;
    this.#disposed = true;
    this.#ready = false;
    this.#stopView();
    this.#status.dispose();
    this.port.dispose();
    this.#input.dispose();
    this.#drag.dispose();
    this.#media.dispose();
    this.#stopTransfer();
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
  #displayPlan(): CanvasDisplayPlan {
    const { address, selection, views } = this.options;
    return canvasDisplayPlan(
      views.read(address.viewId),
      this.#catalog,
      selection.items(address.viewId, this.#catalog),
      selection.preview(address.viewId, this.#catalog),
    );
  }

  /** Prune independent cameras against current output slots. */
  #enroll(): void {
    const slots = this.#catalog.entries.flatMap((entry) =>
      entry.items.map((item) => ({ sourceId: entry.source.id, batchIndex: item.batchIndex })),
    );
    for (const slot of [this.#plan.primary, this.#plan.secondary])
      if (slot?.sourcePresent) slots.push(slot.selection);
    this.options.inspections.synchronize(
      this.options.address.viewId,
      this.#catalog.generation,
      slots,
    );
  }

  /** Reconcile an actual decoded route, including the outgoing image during pending replacement. */
  #resourcesChanged(): void {
    if (!this.#ready || this.#disposed) return;
    this.#browserResources.measured();
    const images = this.#browserResources.resources.read();
    this.#overview.images(images);
    if (this.#plan.view.mode === 'overview') {
      this.#images.images(null, null);
      this.#drag.reconcile();
      this.#input.cancel();
      this.#camera.suspend();
      this.#status.changed();
      return;
    }
    const primary = images.find((slot) => slot.slot === 'primary')?.frame ?? null;
    const secondary = images.find((slot) => slot.slot === 'secondary')?.frame ?? null;
    this.#images.images(primary, secondary);
    this.#drag.reconcile();
    const basis = primary ?? secondary;
    const surface = this.#surface;
    const geometry =
      surface?.visible && basis
        ? canvasCameraGeometry(
            basis.dimensions,
            surface.viewport,
            surface.physicalPixelsPerCssPixel,
          )
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
  #resize(surface: CanvasSurfaceGeometry): void {
    this.#surface = surface;
    this.#drag.reconcile();
    this.#browserResources.active(surface.visible);
    this.#overview.resize();
    this.#resourcesChanged();
  }

  /** Publish the current actual display at bounded cadence, without graph access. */
  #publish(): void {
    if (this.#ready && !this.#disposed) {
      this.#media.reconcile();
      this.port.publish(this.#snapshot());
    }
  }

  /** Report actual camera/fidelity rather than the incoming selection's unshown dimensions. */
  #snapshot() {
    return browserCanvasSnapshot(
      this.options.address,
      this.#plan,
      this.#catalog,
      this.#browserResources.resources.read(),
      this.#camera.read(),
      this.options.connected(),
      this.#browserResources.previewStarted,
      this.#sources,
      this.#browserResources.preparing,
      this.#media.profile(),
    );
  }
}
