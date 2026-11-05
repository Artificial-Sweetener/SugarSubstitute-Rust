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

import type { ComfyWorkflowExecutionRuntime } from '../../execution/infrastructure/ComfyWorkflowExecutionRuntime.js';
import { resolveComfyRendererMode } from '../../core/ComfyRendererMode.js';
import { prefersNativeOutputSurface } from '../../core/embedded/NativePanelTransport.js';
import { OutputPresenterSelection } from '../application/OutputPresenterSelection.js';
import { CanvasViewRegistry } from '../application/CanvasViewRegistry.js';
import { CanvasSourceInventory } from '../application/CanvasSourceInventory.js';
import { CanvasSelectionController } from '../application/CanvasSelectionController.js';
import { CanvasInspectionStore } from '../application/CanvasInspectionStore.js';
import { WebCanvasAdapter } from '../application/WebCanvasAdapter.js';
import { CanvasSessionCoordinator } from '../application/CanvasSessionCoordinator.js';
import { windowId } from '../domain/CanvasIdentity.js';
import { isRecord } from '../../types/common.js';
import { NativeCanvasAttachment } from './NativeCanvasAttachment.js';
import { nativeCanvasRendererKey } from './NativeCanvasCapabilities.js';
import type { CanvasAttachmentOptions } from './CanvasAttachmentOptions.js';
import type { ComfyBrowserCanvasHost } from './ComfyBrowserCanvasHost.js';
import { ComfyCanvasSourceInventory } from './ComfyCanvasSourceInventory.js';
import { ComfyCanvasPreferences } from './ComfyCanvasPreferences.js';
import type { WorkflowViewSource } from '../../workflow/domain/WorkflowView.js';
import { ComfyCanvasWorkspaceHost } from './ComfyCanvasWorkspaceHost.js';
import { CanvasPrimarySurface } from './CanvasPrimarySurface.js';
import { ComfyCanvasToolbar } from './ComfyCanvasToolbar.js';
import { ensureCanvasWorkspaceStyles } from './CanvasWorkspaceStyles.js';
import { CanvasRevealCoordinator } from '../application/CanvasRevealCoordinator.js';
import type { CanvasRevealTarget } from '../domain/CanvasReveal.js';
import { ComfyImageRevealHost } from './ComfyImageRevealHost.js';
import { BrowserOutputTransfer } from '../../output_saving/BrowserOutputTransfer.js';
import { BrowserImageClipboard } from '../../output_saving/BrowserImageClipboard.js';
import { BrowserImageDownload } from '../../output_saving/BrowserImageDownload.js';
import type { OutputSavingFeature } from '../../output_saving/OutputSavingFeature.js';
import { NativeSourceSavingHost } from '../../output_saving/NativeSourceSavingHost.js';
import type { MemoryBudgetRegistry } from '../../core/memory/MemoryBudgetRegistry.js';
import type { CanvasRoutePort } from '../domain/CanvasRoute.js';

export interface ComfyCanvasHostOptions {
  readonly saving: OutputSavingFeature | null;
  readonly presentation: WorkflowViewSource;
  /** Editor width beside the canvas for workflows without a stored width, in CSS pixels. */
  readonly defaultEditorWidth: number;
  readonly app: unknown;
  readonly document: Document;
  readonly execution: ComfyWorkflowExecutionRuntime;
  readonly logger: Pick<Console, 'error'>;
  /** Page allowance shared by browser canvas images and posters. */
  readonly memory: MemoryBudgetRegistry;
  /** The workflow-surface Input/Output route shown in the canvas tab controls. */
  readonly route?: CanvasRoutePort;
  report(error: unknown): void;
}

/** Keep source, selection, resource, frame, settings, and execution ownership explicit in a thin composition. */
export class ComfyCanvasHost {
  readonly #sources: ComfyCanvasSourceInventory;
  readonly #adapter: WebCanvasAdapter<Blob>;
  readonly #coordinator: CanvasSessionCoordinator;
  readonly #preferences: ComfyCanvasPreferences;
  readonly #workspace: ComfyCanvasWorkspaceHost;
  readonly #primary: CanvasPrimarySurface;
  readonly #toolbar: ComfyCanvasToolbar;
  readonly #reveal: CanvasRevealCoordinator;
  readonly #imageActions: ComfyImageRevealHost;
  readonly #saving: NativeSourceSavingHost | null;
  readonly #transfers: BrowserOutputTransfer;
  #browser: ComfyBrowserCanvasHost | null = null;
  readonly #stops: (() => void)[] = [];
  #started = false;
  #disposed = false;

  /** Allocate lightweight owners immediately and defer public host access until extension setup. */
  constructor(private readonly options: ComfyCanvasHostOptions) {
    const { document, execution, app, logger, report, saving } = options;
    const window = document.defaultView;
    if (!window) throw new Error('Output canvas window is unavailable.');
    const transfers = (this.#transfers = new BrowserOutputTransfer(
      document.location.origin,
      (input, init) => window.fetch(input, init),
      new BrowserImageClipboard(document),
      new BrowserImageDownload(document),
    ));
    const nextKey = (): string =>
      [...window.crypto.getRandomValues(new Uint32Array(4))]
        .map((word) => word.toString(16).padStart(8, '0'))
        .join('');
    const frames = {
      now: () => window.performance.now(),
      request: (callback: (time: number) => void) => {
        const id = window.requestAnimationFrame(callback);
        return () => window.cancelAnimationFrame(id);
      },
    };
    const clock = {
      now: () => window.performance.now(),
      later: (delay: number, callback: () => void) => {
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
    this.#sources = new ComfyCanvasSourceInventory(
      execution.sessions,
      inventory,
      () => (isRecord(app) ? app.graph : null),
      logger,
    );
    this.#saving = options.saving
      ? new NativeSourceSavingHost(
          options.saving,
          {
            root: () => (isRecord(app) ? app.graph : null),
            canvas: () => (isRecord(app) ? app.canvas : null),
            scope: () => execution.sessions.read().active,
            sources: () => this.#sources.savingSources(),
          },
          report,
          { document, subscribe: (listener) => this.#sources.subscribe(listener) },
        )
      : null;
    this.#adapter = new WebCanvasAdapter({
      wants: (frame) => this.#primary.wants(frame.source),
      receive: (frame) => this.#primary.receive(frame.preview, frame.bytes),
    });
    this.#coordinator = new CanvasSessionCoordinator(
      execution.sessions,
      this.#adapter,
      inventory,
      views,
      selection,
      this.#sources,
    );
    this.#stops.push(
      this.#coordinator.onRetired((view) => {
        inspections.remove(view);
        this.#browser?.remove(view);
      }),
    );
    this.#preferences = new ComfyCanvasPreferences(app);
    const changed = (): void => {
      if (this.#started && !this.#disposed) {
        this.#primary.reconcile();
        this.#toolbar.update(this.#workspace.visible, this.#workspace.content !== null);
      }
    };
    this.#workspace = new ComfyCanvasWorkspaceHost(
      app,
      document,
      options.defaultEditorWidth,
      frames,
      changed,
      report,
      options.presentation,
    );
    const presenter = new OutputPresenterSelection({
      prefersNative: () => prefersNativeOutputSurface(window),
      surface: () => execution.canvases.surfaceHost('embedded-document'),
      discovered: () => execution.canvasSurfaceDiscovered,
    });
    const nativePresenter = (): boolean => presenter.current() !== 'browser';
    this.#primary = new CanvasPrimarySurface({
      document,
      window: windowId(nextKey()),
      views,
      coordinator: this.#coordinator,
      element: () => this.#workspace.canvas,
      renderer: () =>
        `${resolveComfyRendererMode(app, null, document)}:${nativePresenter() ? nativeCanvasRendererKey(execution.canvases.surfaceHost('embedded-document')) : 'browser'}`,
      report,
      clock,
      mount: (element, address, _catalog, accepts, acceptsPreparation, recover) => {
        const shared: CanvasAttachmentOptions = {
          address,
          views,
          selection,
          inspections,
          clock,
          frames,
          accepts,
          catalog: () => this.#coordinator.catalog(address.viewId),
          connected: () => execution.connected,
          copy: (item, accepts, format, signal) =>
            transfers.copy(item.file, accepts, format, signal),
          fileAction: (item, accepts, signal) =>
            transfers.fileAction(
              item.file,
              accepts,
              signal,
              item.mediaKind === 'image' ? 'image' : 'media',
            ),
          mediaProfile: () => transfers.profile(),
          photoshop: (document, layers, accepts, signal) =>
            transfers.photoshop(
              document,
              layers.map((layer) => ({ file: layer.item.file, name: layer.name })),
              accepts,
              signal,
            ),
          drag: (item, signal) => transfers.drag(item.file, signal),
          transferChanged: (listener: () => void) => {
            const profile = transfers.subscribe(listener);
            const settings = saving?.configuration.subscribe(listener);
            return () => {
              profile();
              settings?.();
            };
          },
          report: (error: unknown) => report(error),
          ...(options.route ? { route: options.route } : {}),
        };
        if (nativePresenter())
          return new NativeCanvasAttachment(element, {
            ...shared,
            acceptsPreparation,
            nativeSurface: () => execution.canvases.surfaceHost('embedded-document'),
            surfacePending: () => presenter.current() === 'pending',
            recover,
            editing: () => execution.canvases.editHost(),
          });
        return import('./ComfyBrowserCanvasHost.js').then(({ ComfyBrowserCanvasHost }) => {
          if (!acceptsPreparation()) return null;
          const browser = (this.#browser ??= new ComfyBrowserCanvasHost(
            document,
            logger,
            options.memory,
          ));
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
  get content(): HTMLElement | null {
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
  setup(): void {
    if (this.#started || this.#disposed) return;
    try {
      this.#preferences.setup();
      this.#sources.install();
      this.#stops.push(this.options.execution.canvases.register(this.#adapter.adapter));
      this.#coordinator.install();
      this.#started = true;
      void this.#transfers.refresh().catch(this.options.report);
      if (this.options.saving)
        this.#stops.push(
          this.options.saving.configuration.subscribe(() => {
            void this.#transfers.refresh().catch(this.options.report);
          }),
        );
      ensureCanvasWorkspaceStyles(this.options.document);
      this.#primary.setup();
      this.#workspace.setup();
      this.#toolbar.setup();
      this.#imageActions.setup();
      this.#saving?.setup();
      this.#stops.push(this.options.execution.subscribeConnection(() => this.#primary.reconcile()));
      this.options.document.addEventListener(
        'sugarcubes-canvas-discovered',
        this.#canvasDiscovered,
      );
      this.options.document.defaultView?.addEventListener('pagehide', this.#pagehide);
    } catch (error: unknown) {
      this.dispose();
      this.options.report(error);
    }
  }

  /** Use the same command owner in graph and masonry presentation. */
  toggle(): void {
    this.#workspace.toggle();
  }

  /** Reveal a provenance-qualified completed thumbnail through the shared canvas authority. */
  async reveal(target: CanvasRevealTarget): Promise<boolean> {
    if (this.#disposed || !this.#started) return false;
    return await this.#reveal.reveal(target);
  }

  /** Preserve qualified history pixels and retire primary resources before graph replacement. */
  beforeConfigureGraph(): void {
    const loading = this.options.execution.sessions.read().loading;
    this.#workspace.beforeConfigureGraph(loading && this.#coordinator.active() !== null);
  }

  /** Restore a qualified allocation before masonry resumes borrowed native controls. */
  afterConfigureGraph(): void {
    this.#workspace.afterConfigureGraph();
  }

  /** Dispose this browser composition without disposing shared execution or other future window owners. */
  dispose(): void {
    if (this.#disposed) return;
    this.#disposed = true;
    this.options.document.defaultView?.removeEventListener('pagehide', this.#pagehide);
    this.options.document.removeEventListener(
      'sugarcubes-canvas-discovered',
      this.#canvasDiscovered,
    );
    this.#primary.dispose();
    this.#imageActions.dispose();
    this.#saving?.dispose();
    this.#workspace.dispose();
    this.#toolbar.dispose();
    this.#coordinator.dispose();
    for (const stop of this.#stops.splice(0)) stop();
    this.#adapter.dispose();
    this.#sources.dispose();
    this.#transfers.dispose();
    this.#browser?.dispose();
  }

  /** Honor actual page teardown without introducing a recurring visibility or frame loop. */
  readonly #pagehide = (): void => this.dispose();
  /** Reselect the presenter and surface after every concluded native discovery. */
  readonly #canvasDiscovered = (): void => this.#primary.reconcile();
}
