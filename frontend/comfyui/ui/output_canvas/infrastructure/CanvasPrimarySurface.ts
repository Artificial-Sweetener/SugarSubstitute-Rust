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
/** Retire inactive primary attachments while retaining per-workflow lightweight view state. */

import type { CanvasCatalogSnapshot } from '../domain/CanvasCatalog.js';
import type { CanvasAttachmentAddress, CanvasViewId, WindowId } from '../domain/CanvasIdentity.js';
import type { CanvasViewRegistry } from '../application/CanvasViewRegistry.js';
import type { CanvasSessionCoordinator } from '../application/CanvasSessionCoordinator.js';
import type { WorkflowPreviewConsumer } from '../../execution/infrastructure/ComfyWorkflowPreviewDelivery.js';
import type { WorkflowPreview } from '../../execution/domain/WorkflowPreview.js';
import type { AttributedWorkflowSource } from '../../execution/application/WorkflowExecutionIngestor.js';
import type { CanvasImageDiagnostics } from '../domain/CanvasImageResource.js';
import type { CanvasRevealTarget } from '../domain/CanvasReveal.js';
import type { CanvasRevealDestination } from '../application/CanvasRevealCoordinator.js';

export interface PrimaryCanvasAttachment {
  /** Settle within the host's bounded readiness deadline; disposal must also settle this promise. */
  readonly ready?: Promise<void>;
  /** Read counters only on explicit diagnostic request, with no telemetry timer. */
  diagnostics(): CanvasImageDiagnostics & { readonly renderFrames: number };
  refresh(): void;
  reveal?(target: CanvasRevealTarget): Promise<boolean>;
  wants(
    session: WorkflowPreview['sessionId'],
    generation: number,
    source: WorkflowPreview['sourceId'],
  ): boolean;
  receive(preview: WorkflowPreview, blob: Blob): void;
  /** Revoke immediately; settle asynchronous native release before another attachment mounts. */
  dispose(): void | Promise<void>;
}

interface AttachmentRecord {
  readonly view: CanvasViewId;
  readonly generation: number;
  readonly element: HTMLElement;
  readonly renderer: string;
  readonly address: CanvasAttachmentAddress;
  readonly root: HTMLElement;
  readonly attachment: PrimaryCanvasAttachment;
}

interface PendingAttachment extends Omit<AttachmentRecord, 'attachment'> {
  attachment: PrimaryCanvasAttachment | null;
  mounting: boolean;
}

export interface CanvasPrimarySurfaceOptions {
  readonly document: Document;
  readonly window: WindowId;
  readonly views: CanvasViewRegistry;
  readonly coordinator: CanvasSessionCoordinator;
  element(): HTMLElement | null;
  renderer(): string;
  /** Separate staged preparation authority from committed view and preview delivery. */
  mount(
    element: HTMLElement,
    address: CanvasAttachmentAddress,
    catalog: CanvasCatalogSnapshot,
    accepts: () => boolean,
    acceptsPreparation: () => boolean,
  ): PrimaryCanvasAttachment | Promise<PrimaryCanvasAttachment | null>;
  report(error: unknown): void;
}

/** Give exactly one active visible primary workflow ownership of its surface and ephemeral previews. */
export class CanvasPrimarySurface implements WorkflowPreviewConsumer {
  #current: AttachmentRecord | null = null;
  #pending: PendingAttachment | null = null;
  #stop: (() => void) | null = null;
  #disposed = false;
  #retirement: Promise<void> | null = null;

  /** Inject attachment construction so lifecycle proofs exercise authority without borrowing platform decoders. */
  constructor(private readonly options: CanvasPrimarySurfaceOptions) {}

  /** Report this primary attachment's bounded owned resources without exposing handles or workflow files. */
  diagnostics() {
    return {
      address: this.#current?.address ?? null,
      generation: this.#current?.generation ?? null,
      resources: this.#current?.attachment.diagnostics() ?? null,
    };
  }

  /**
   * Return the view's committed attachment as its renderer-neutral inspection destination, or
   * the staged candidate still preparing for that view when none is committed. A candidate
   * decides itself whether it can retain a reveal until it commits.
   */
  destination(view: CanvasViewId): CanvasRevealDestination | null {
    this.reconcile();
    const attachment =
      this.#current?.view === view
        ? this.#current.attachment
        : this.#pending?.view === view
          ? this.#pending.attachment
          : null;
    if (!attachment?.reveal) return null;
    const reveal = attachment.reveal.bind(attachment);
    return { reveal };
  }

  /** Observe scoped catalog boundaries and document visibility without another execution event listener. */
  setup(): void {
    if (this.#stop || this.#disposed) return;
    this.#stop = this.options.coordinator.subscribe((view) => {
      if (view === null || view === this.options.coordinator.active()) this.reconcile();
    });
    this.options.document.addEventListener('visibilitychange', this.reconcile);
    this.reconcile();
  }

  /** Keep a same-view surface until its replacement is ready; never show a different workflow. */
  readonly reconcile = (): void => {
    if (this.#disposed) return;
    const { coordinator, views } = this.options;
    const view = coordinator.active();
    const element = !this.options.document.hidden ? this.options.element() : null;
    const catalog = view && element ? coordinator.catalog(view) : null;
    const renderer = this.options.renderer();
    if (!view || !element || !catalog) {
      this.#cancelPending();
      this.#release();
      return;
    }
    if (
      this.#current?.view === view &&
      this.#current.generation === catalog.generation &&
      this.#current.element === element &&
      this.#current.renderer === renderer
    ) {
      this.#cancelPending();
      this.#current.attachment.refresh();
      return;
    }
    if (
      this.#pending?.view === view &&
      this.#pending.generation === catalog.generation &&
      this.#pending.element === element &&
      this.#pending.renderer === renderer
    ) {
      if (!this.#pending.mounting && !this.#retirement) this.#mount(this.#pending, catalog);
      else this.#pending.attachment?.refresh();
      this.#current?.attachment.refresh();
      return;
    }
    this.#cancelPending();
    if (
      this.#current &&
      (this.#current.view !== view ||
        this.#current.generation !== catalog.generation ||
        this.#current.element !== element)
    )
      this.#release();
    const root = this.options.document.createElement('div');
    root.style.position = 'absolute';
    root.style.inset = '0';
    root.style.visibility = 'hidden';
    root.style.pointerEvents = 'none';
    let address: CanvasAttachmentAddress;
    try {
      address = views.prepareAttachment(view, this.options.window);
      try {
        element.append(root);
      } catch (error: unknown) {
        views.cancelAttachment(address);
        throw error;
      }
    } catch (error: unknown) {
      root.remove();
      this.options.report(error);
      return;
    }
    const pending: PendingAttachment = {
      view,
      generation: catalog.generation,
      element,
      renderer,
      address,
      root,
      attachment: null,
      mounting: false,
    };
    this.#pending = pending;
    if (!this.#retirement) this.#mount(pending, catalog);
  };

  /** Construct only the candidate that still owns the staged registry lease. */
  #mount(pending: PendingAttachment, catalog: CanvasCatalogSnapshot): void {
    if (this.#pending !== pending || this.#disposed || pending.mounting) return;
    pending.mounting = true;
    const { views, coordinator } = this.options;
    const { address, view, element, renderer, root } = pending;
    try {
      const accepts = (): boolean =>
        !this.#disposed &&
        views.accepts(address) &&
        coordinator.active() === view &&
        coordinator.catalog(view)?.generation === catalog.generation &&
        !this.options.document.hidden &&
        this.options.element() === element;
      const acceptsPreparation = (): boolean =>
        !this.#disposed &&
        this.#pending === pending &&
        views.acceptsPreparation(address) &&
        coordinator.active() === view &&
        coordinator.catalog(view)?.generation === catalog.generation &&
        !this.options.document.hidden &&
        this.options.element() === element &&
        this.options.renderer() === renderer;
      if (!acceptsPreparation()) {
        this.#cancelPending();
        return;
      }
      const attachment = this.options.mount(root, address, catalog, accepts, acceptsPreparation);
      if (attachment instanceof Promise)
        void attachment.then(
          (loaded) => this.#adopt(pending, loaded),
          (error: unknown) => this.#failPending(pending, error),
        );
      else this.#adopt(pending, attachment);
    } catch (error: unknown) {
      this.#failPending(pending, error);
    }
  }

  /** Refuse inactive preview bytes synchronously, before any object URL or decode is created. */
  wants(source: AttributedWorkflowSource): boolean {
    return (
      this.#current?.attachment.wants(
        source.run.session.sessionId,
        source.run.session.generation,
        source.source.id,
      ) ?? false
    );
  }

  /** Deliver only the current attachment's qualified preview without retaining the Blob. */
  receive(preview: WorkflowPreview, blob: Blob): void {
    this.#current?.attachment.receive(preview, blob);
  }

  /** Retire subscriptions and pixels while leaving shared execution and other canvas views intact. */
  dispose(): void {
    if (this.#disposed) return;
    this.#disposed = true;
    this.#stop?.();
    this.#stop = null;
    this.options.document.removeEventListener('visibilitychange', this.reconcile);
    this.#cancelPending();
    this.#release();
  }

  /** Adopt only the still-current constructed attachment, retiring late loads immediately. */
  #adopt(pending: PendingAttachment, attachment: PrimaryCanvasAttachment | null): void {
    if (!attachment) {
      if (this.#pending === pending) {
        this.#cancelPending();
        this.reconcile();
      }
      return;
    }
    if (this.#pending !== pending) {
      void attachment.ready?.catch(() => {});
      try {
        this.#disposeAttachment(attachment);
      } catch (error: unknown) {
        this.options.report(error);
      } finally {
        pending.root.remove();
      }
      return;
    }
    pending.attachment = attachment;
    if (attachment.ready)
      void attachment.ready.then(
        () => this.#activate(pending),
        (error: unknown) => this.#failPending(pending, error),
      );
    else this.#activate(pending);
  }

  /** Commit only a still-current ready candidate, then retire the replaced surface. */
  #activate(pending: PendingAttachment): void {
    if (this.#pending !== pending) return;
    const { coordinator, views } = this.options;
    if (
      this.#disposed ||
      this.options.document.hidden ||
      coordinator.active() !== pending.view ||
      coordinator.catalog(pending.view)?.generation !== pending.generation ||
      this.options.element() !== pending.element ||
      this.options.renderer() !== pending.renderer ||
      !pending.attachment
    ) {
      this.#cancelPending();
      this.reconcile();
      return;
    }
    try {
      views.commitAttachment(pending.address);
    } catch (error: unknown) {
      this.#failPending(pending, error);
      return;
    }
    const previous = this.#current;
    this.#pending = null;
    this.#current = { ...pending, attachment: pending.attachment };
    pending.root.style.visibility = '';
    pending.root.style.pointerEvents = '';
    if (previous) {
      try {
        this.#retire(previous);
      } catch (error: unknown) {
        this.options.report(error);
      }
    }
    try {
      pending.attachment.refresh();
    } catch (error: unknown) {
      this.options.report(error);
    }
  }

  /** Ignore failures from a canceled candidate and retain the previous committed attachment. */
  #failPending(pending: PendingAttachment, error: unknown): void {
    if (this.#pending !== pending) return;
    this.#cancelPending();
    this.options.report(error);
  }

  /** Cancel the exact staged transaction before disposing any candidate resources. */
  #cancelPending(): void {
    const pending = this.#pending;
    if (!pending) return;
    this.#pending = null;
    this.options.views.cancelAttachment(pending.address);
    try {
      if (pending.attachment) this.#disposeAttachment(pending.attachment);
    } catch (error: unknown) {
      this.options.report(error);
    } finally {
      pending.root.remove();
    }
  }

  /** Clear routing before surface disposal so late decoder or preview callbacks cannot regain ownership. */
  #release(): void {
    const previous = this.#current;
    this.#current = null;
    if (previous) this.#retire(previous);
  }

  /** Release only a surface's own DOM and lease, even after a replacement commits. */
  #retire(previous: AttachmentRecord): void {
    try {
      this.#disposeAttachment(previous.attachment);
    } finally {
      previous.root.remove();
      this.options.views.releaseAttachment(previous.address);
    }
  }

  /** Retain failed release as a fence: uncertain native custody cannot authorize another mount. */
  #disposeAttachment(attachment: PrimaryCanvasAttachment): void {
    let release: void | Promise<void>;
    try {
      release = attachment.dispose();
    } catch (error: unknown) {
      release = Promise.reject(
        error instanceof Error
          ? error
          : new Error('Canvas attachment release failed.', { cause: error }),
      );
    }
    if (!release) return;
    const retirement = this.#retirement
      ? Promise.all([this.#retirement, release]).then(() => {})
      : release;
    this.#retirement = retirement;
    void retirement.then(
      () => {
        if (this.#retirement !== retirement) return;
        this.#retirement = null;
        try {
          if (this.#pending) this.reconcile();
          else this.#current?.attachment.refresh();
        } catch (error: unknown) {
          this.options.report(error);
        }
      },
      (error: unknown) => {
        if (this.#pending) this.#failPending(this.#pending, error);
        else this.options.report(error);
      },
    );
  }
}
