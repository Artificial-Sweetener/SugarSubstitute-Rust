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
/** Retire inactive primary browser attachments while retaining per-workflow lightweight view state. */

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
  dispose(): void;
}

export interface CanvasPrimarySurfaceOptions {
  readonly document: Document;
  readonly window: WindowId;
  readonly views: CanvasViewRegistry;
  readonly coordinator: CanvasSessionCoordinator;
  element(): HTMLElement | null;
  renderer(): string;
  mount(
    element: HTMLElement,
    address: CanvasAttachmentAddress,
    catalog: CanvasCatalogSnapshot,
    accepts: () => boolean,
  ): PrimaryCanvasAttachment;
  report(error: unknown): void;
}

/** Give exactly one active visible primary workflow ownership of browser pixels and ephemeral preview bytes. */
export class CanvasPrimarySurface implements WorkflowPreviewConsumer {
  #current: {
    view: CanvasViewId;
    generation: number;
    element: HTMLElement;
    renderer: string;
    address: CanvasAttachmentAddress;
    attachment: PrimaryCanvasAttachment;
  } | null = null;
  #stop: (() => void) | null = null;
  #disposed = false;

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

  /** Return only the active attachment's renderer-neutral inspection destination. */
  destination(view: CanvasViewId): CanvasRevealDestination | null {
    this.reconcile();
    const current = this.#current;
    if (current?.view !== view || !current.attachment.reveal) return null;
    const reveal = current.attachment.reveal.bind(current.attachment);
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

  /** Replace resources before a tab/generation/renderer/allocated-container transition can overlap ownership. */
  readonly reconcile = (): void => {
    if (this.#disposed) return;
    const { coordinator, views } = this.options;
    const view = coordinator.active();
    const element = !this.options.document.hidden ? this.options.element() : null;
    const catalog = view && element ? coordinator.catalog(view) : null;
    const renderer = this.options.renderer();
    if (!view || !element || !catalog) {
      this.#release();
      return;
    }
    if (
      this.#current?.view === view &&
      this.#current.generation === catalog.generation &&
      this.#current.element === element &&
      this.#current.renderer === renderer
    ) {
      this.#current.attachment.refresh();
      return;
    }
    this.#release();
    const address = views.prepareAttachment(view, this.options.window);
    views.commitAttachment(address);
    try {
      const accepts = (): boolean =>
        !this.#disposed &&
        views.accepts(address) &&
        coordinator.active() === view &&
        !this.options.document.hidden &&
        this.options.element() === element &&
        this.options.renderer() === renderer;
      const attachment = this.options.mount(element, address, catalog, accepts);
      this.#current = {
        view,
        element,
        address,
        attachment,
        renderer,
        generation: catalog.generation,
      };
    } catch (error: unknown) {
      views.releaseAttachment(address);
      this.options.report(error);
    }
  };

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
    this.#release();
  }

  /** Clear routing before surface disposal so late decoder or preview callbacks cannot regain ownership. */
  #release(): void {
    const previous = this.#current;
    this.#current = null;
    previous?.attachment.dispose();
    if (previous) this.options.views.releaseAttachment(previous.address);
  }
}
