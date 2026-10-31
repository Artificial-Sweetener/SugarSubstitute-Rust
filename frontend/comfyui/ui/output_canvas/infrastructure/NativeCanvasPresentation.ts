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
/** Hand an existing view's selected result to a negotiated native surface. */

import type { CanvasCommandScope } from '../domain/CanvasViewPort.js';
import {
  sendNativeCanvasOperation,
  releaseNativeCanvasBinding,
  createNativeCanvasBinding,
  type NativeCanvasBinding,
  type NativeCanvasOperation,
} from './NativeCanvasBinding.js';
import type { WorkflowOutputItem } from '../../execution/domain/WorkflowOutput.js';
import type { WorkflowPreview } from '../../execution/domain/WorkflowPreview.js';
import type { ExecutionClock } from '../../execution/domain/WorkflowExecutionEvent.js';
import {
  IDLE_NATIVE_CANVAS_PREVIEW_DIAGNOSTICS,
  NativeCanvasPreview,
} from './NativeCanvasPreview.js';
import { NativeCanvasViewportIntent } from './NativeCanvasViewportIntent.js';
import { nativeCanvasComparison } from './NativeCanvasCapabilities.js';
import type { CanvasAttachmentAddress } from '../domain/CanvasIdentity.js';
import type { CanvasSurfaceGeometry } from '../domain/CanvasViewPort.js';
import type {
  CanvasSurfaceHost,
  CanvasSurfaceOverviewCell,
  CanvasSurfaceViewportCommand,
  CanvasSurfaceViewportState,
  CanvasSurfaceResult,
  CanvasSurfacePresentationDisplay,
} from '../../../../../integrations/contracts/CanvasSurface.js';
import {
  nativeCanvasDesired,
  nativeCanvasNavigable,
  nativeCanvasOverviewKey,
  nativeCanvasSelection,
  nativeCanvasWipe,
  nativeCanvasWipeKey,
  type NativeCanvasComparison,
  type NativeCanvasDesired as Desired,
} from './NativeCanvasDesired.js';

export interface NativeCanvasPresentationOptions {
  readonly address: CanvasAttachmentAddress;
  readonly clock: ExecutionClock;
  /** Resolve the negotiated surface through the shared canvas registry. */
  host(): CanvasSurfaceHost | null;
  /** The view registry and active workflow remain the attachment authority. */
  accepts(): boolean;
  /** Authorize preparation before the view registry commits this exact candidate. */
  acceptsPreparation(): boolean;
  /** Report real native preparation, before committing presentation. */
  prepared(): void;
  /** Reveal acknowledged native pixels, including the retained image during source replacement. */
  visible(value: boolean): void;
  /** Publish only the current host operation's presentation outcome. */
  status(value: NativeCanvasPresentationStatus): void;
  /** Publish only authoritative state for the currently presented source and allocation. */
  viewport(value: CanvasSurfaceViewportState | null): void;
  /** Lock preview controls only for pixels acknowledged by the presentation observation. */
  preview(value: boolean): void;
  report(error: unknown): void;
}

/** Describe native readiness without retaining a browser image or native handle. */
export interface NativeCanvasPresentationStatus {
  readonly state: 'empty' | 'loading' | 'ready' | 'unavailable' | 'error';
  readonly message: string;
}

/** Keep one in-flight operation and coalesce newer view or geometry changes. */
export class NativeCanvasPresentation {
  #binding: NativeCanvasBinding | null = null;
  #viewport: NativeCanvasViewportIntent | null = null;
  #desired: Desired | null = null;
  #running = false;
  #dirty = false;
  #visible = false;
  #disposed = false;
  #retirement: Promise<void> | null = null;
  #preview: NativeCanvasPreview | null = null;
  #viewportRecovery: {
    binding: NativeCanvasBinding;
    desired: Desired;
    presentation: object;
  } | null = null;

  /** Bind native presentation to the existing qualified view address. */
  constructor(private readonly options: NativeCanvasPresentationOptions) {}

  /**
   * Reconcile only the selection, comparison, overview and geometry chosen by the view owners.
   * A divider-only comparison change keeps the presented pair and its camera; the newest
   * divider follows any in-flight placement rather than cancelling it. Overview layout changes
   * likewise follow the overview request in flight.
   */
  update(
    geometry: CanvasSurfaceGeometry | null,
    item: WorkflowOutputItem | null,
    preview: WorkflowPreview | null = null,
    comparison: NativeCanvasComparison | null = null,
    overview: readonly CanvasSurfaceOverviewCell[] | null = null,
  ): void {
    if (this.#disposed) return;
    this.#viewportRecovery = null;
    const preparing = this.options.acceptsPreparation();
    const host = this.options.accepts() || preparing ? this.options.host() : null;
    if (host !== (this.#binding?.host ?? null)) {
      void this.#retire();
      if (host) {
        this.#binding = createNativeCanvasBinding(host);
        const binding = this.#binding;
        this.#preview = new NativeCanvasPreview({
          address: this.options.address,
          host,
          clock: this.options.clock,
          active: () => this.#owns(binding) && this.options.accepts(),
          finalRevision: () => binding.selectionRevision,
          changed: () => this.#wake(),
          cancelOperation: () => {
            if (binding.active?.type.startsWith('surface.preview.'))
              binding.active.cancellation.cancel();
          },
          displayed: (value, presentation) => this.#observed(binding, value, presentation),
          report: this.options.report,
        });
      }
      this.#desired = null;
    }
    const previewChanged = this.#preview?.update(preview) ?? false;
    const desired = nativeCanvasDesired(
      geometry,
      item,
      nativeCanvasComparison(host) ? comparison : null,
      host?.capabilities.overview ? overview : null,
      this.options.accepts(),
      preparing,
    );
    if (this.#desired?.key === desired.key) {
      // Focus and gesture occlusions change routing, never the presented allocation.
      const routing = JSON.stringify(this.#desired.geometry) !== JSON.stringify(geometry);
      const divider =
        nativeCanvasWipeKey(this.#desired.comparison) !== nativeCanvasWipeKey(desired.comparison);
      const layout =
        nativeCanvasOverviewKey(this.#desired.overview) !==
        nativeCanvasOverviewKey(desired.overview);
      if (routing) this.#desired.geometry = geometry;
      this.#desired.comparison = desired.comparison;
      this.#desired.overview = desired.overview;
      if (routing || divider || layout || previewChanged) this.#wake();
      return;
    }
    const retainedFinal =
      desired.previewable &&
      this.#preview?.observing &&
      this.#binding?.selectionRevision !== null &&
      this.#binding?.geometry === JSON.stringify(geometry);
    this.#desired = desired;
    if (!retainedFinal) this.options.viewport(null);
    if (this.#viewport && !retainedFinal) this.#cancelViewport(this.#viewport);
    // A previous allocation can no longer be presented after the native window resizes.
    // Cancel its acknowledgement wait so the current geometry can reach the host.
    if (this.#binding?.active?.dependsOnView && !retainedFinal)
      this.#binding.active.cancellation.cancel();
    // The native owner retains acknowledged pixels while a same-allocation replacement loads.
    if (
      (!desired.shown && !(desired.previewable && this.#preview?.observing)) ||
      !this.#binding?.committed ||
      this.#binding.geometry !== JSON.stringify(geometry)
    )
      this.#show(false);
    this.options.status(
      desired.shown
        ? host
          ? { state: 'loading', message: 'Loading the selected image.' }
          : { state: 'unavailable', message: 'The native canvas is unavailable.' }
        : { state: 'empty', message: '' },
    );
    this.#wake();
  }

  /** Refuse preview bytes until a committed, observed binding owns the logical interest. */
  wants(): boolean {
    return this.#preview?.wants() ?? false;
  }

  /** Forward immutable encoded bytes without constructing a second renderer or mutation queue. */
  receive(preview: WorkflowPreview, blob: Blob): void {
    this.#preview?.receive(preview, blob);
  }

  /** Expose only local encoded custody; native decoded pixels remain host-owned. */
  diagnostics() {
    return this.#preview?.diagnostics() ?? IDLE_NATIVE_CANVAS_PREVIEW_DIAGNOSTICS;
  }

  /** Report implemented live preview support independently of whether pixels have arrived. */
  get previewAvailable(): boolean {
    return this.#preview?.observing ?? false;
  }

  /** Admit one discrete intent into the existing presentation stream, replacing only older intent. */
  viewport(command: CanvasSurfaceViewportCommand, scope: CanvasCommandScope): Promise<boolean> {
    const binding = this.#binding;
    if (
      !binding ||
      !nativeCanvasNavigable(this.#desired, this.#preview, binding.selectionRevision) ||
      !binding.host.capabilities.viewportState ||
      !scope.current() ||
      command.type === 'read' ||
      !binding.host.capabilities.viewportCommands.includes(command.type)
    )
      return Promise.resolve(false);
    const previous = this.#viewport;
    if (previous) this.#cancelViewport(previous);
    const intent = new NativeCanvasViewportIntent(
      command,
      scope,
      (cancelled) => this.#cancelViewport(cancelled),
      (finished) => {
        if (this.#viewport === finished) this.#viewport = null;
      },
    );
    this.#viewport = intent;
    intent.observe();
    if (this.#viewport === intent) this.#wake();
    return intent.result;
  }

  /** Keep a forwarded operation awaiting native acknowledgement; withdraw unsent intent locally. */
  #cancelViewport(intent: NativeCanvasViewportIntent): void {
    if (intent.running) this.#binding?.active?.cancellation.cancel();
    else intent.complete(false);
  }

  /** Revoke the exact surface before releasing its transport and late completions. */
  dispose(): Promise<void> {
    if (this.#disposed) return this.#retirement ?? Promise.resolve();
    this.#disposed = true;
    this.#desired = null;
    return this.#retire();
  }

  /** Mark the desired state changed and resume the operation stream if it is idle. */
  #wake(): void {
    this.#dirty = true;
    this.#start();
  }

  #start(): void {
    if (this.#running || this.#retirement || !this.#binding || !this.#desired || this.#disposed)
      return;
    this.#running = true;
    void this.#run().finally(() => {
      this.#running = false;
      if (this.#dirty && !this.#disposed) this.#start();
    });
  }

  /**
   * Converge the bound surface on the newest desired state, one native mutation at a time.
   * Each pass prepares, places, selects and commits in order, then places a comparison divider,
   * serves viewport intent and finally one preview transfer step; a change during any await
   * marks the run dirty again. A newer divider never cancels an in-flight placement, so one
   * placement is in flight and only the newest value waits behind it.
   */
  async #run(): Promise<void> {
    while (this.#dirty && !this.#disposed) {
      this.#dirty = false;
      const binding = this.#binding;
      const desired = this.#desired;
      if (!binding || !desired || !this.#owns(binding)) return;
      try {
        if (
          (desired.shown ||
            (desired.empty &&
              (this.options.acceptsPreparation() ||
                (desired.previewable && this.#preview?.observing)))) &&
          desired.geometry &&
          !binding.prepared
        ) {
          const result = await this.#send(binding, { type: 'surface.prepare' });
          if (!this.#owns(binding)) return;
          if (result.status !== 'ready') {
            this.#failed(desired, result);
            continue;
          }
          binding.prepared = true;
        }
        if (!this.#current(binding, desired)) continue;
        const lastGeometry = desired.geometry ?? binding.lastGeometry;
        if (!binding.prepared || !lastGeometry) continue;
        const geometry =
          desired.shown || (desired.previewable && this.#preview?.observing)
            ? lastGeometry
            : { ...lastGeometry, visible: false, clip: { x: 0, y: 0, width: 0, height: 0 } };
        const geometryKey = JSON.stringify(geometry);
        if (geometryKey !== binding.geometry) {
          const result = await this.#send(binding, { type: 'surface.geometry', geometry });
          if (!this.#owns(binding)) return;
          if (result.status !== 'applied') {
            this.#failed(desired, result);
            continue;
          }
          binding.geometry = geometryKey;
          binding.lastGeometry = geometry;
        }
        if (!this.#current(binding, desired)) continue;
        if (!desired.shown) {
          if (!desired.previewable || !this.#preview?.observing) {
            if (desired.empty) this.options.prepared();
            continue;
          }
        }
        const selection = nativeCanvasSelection(desired);
        if (selection && selection.key !== binding.selection) {
          const preview = this.#preview;
          preview?.selecting();
          let selectionRevision = 0;
          const result = await this.#send(binding, selection.operation, (revision) => {
            selectionRevision = revision;
          }).finally(() => preview?.selectionFinished());
          if (!this.#owns(binding)) return;
          if (result.status !== 'applied') {
            this.#failed(desired, result);
            continue;
          }
          binding.selection = selection.key;
          binding.selectionRevision = selectionRevision;
          preview?.selected();
        }
        if (!this.#current(binding, desired)) continue;
        this.options.prepared();
        if (!this.options.accepts()) continue;
        if (!binding.committed) {
          const result = await this.#send(binding, { type: 'surface.commit' });
          if (!this.#owns(binding)) return;
          if (result.status !== 'applied') {
            this.#failed(desired, result);
            continue;
          }
          binding.committed = true;
        }
        if (!this.#current(binding, desired)) continue;
        const wipe = nativeCanvasWipe(desired);
        if (wipe && wipe.key !== binding.wipe) {
          const result = await this.#send(binding, {
            type: 'surface.viewport.request',
            command: wipe.command,
          });
          if (!this.#owns(binding)) return;
          const current = this.#current(binding, desired);
          if (result.status === 'applied' && 'viewport' in result) {
            // The host placed this divider even when a newer one is now pending behind it.
            binding.wipe = wipe.key;
            if (current) {
              binding.viewportKey = desired.key;
              this.options.viewport(result.viewport);
            }
          } else if (result.status === 'cancelled') this.#dirty ||= current;
          else this.#failed(desired, result);
          if (!current || binding.wipe !== wipe.key) continue;
        }
        const intent = this.#viewport;
        if (intent && !intent.scope.current()) intent.complete(false);
        const admitted = this.#viewport;
        if (
          admitted ||
          ((desired.item ||
            (desired.previewable &&
              this.#preview?.observing &&
              binding.selectionRevision !== null)) &&
            binding.host.capabilities.viewportState &&
            binding.viewportKey !== desired.key)
        ) {
          if (admitted) {
            admitted.running = true;
            binding.viewportKey = null;
            this.options.viewport(null);
          }
          const displayEpoch = this.#preview?.epoch;
          try {
            const result = await this.#send(binding, {
              type: 'surface.viewport.request',
              command: admitted?.command ?? { type: 'read' },
            });
            const current =
              this.#current(binding, desired) &&
              displayEpoch === this.#preview?.epoch &&
              (!admitted || (this.#viewport === admitted && admitted.scope.current()));
            if (current && result.status === 'applied' && 'viewport' in result) {
              binding.viewportKey = desired.key;
              this.options.viewport(result.viewport);
              admitted?.complete(true);
            } else {
              const recoverUnavailable =
                admitted !== null && current && result.status === 'unavailable';
              const presentation = this.#preview?.pendingPresentation;
              if (current && result.status === 'stale' && presentation)
                this.#viewportRecovery = { binding, desired, presentation };
              admitted?.complete(false);
              if (
                admitted &&
                this.#current(binding, desired) &&
                (!current || result.status === 'cancelled' || recoverUnavailable)
              )
                this.#dirty = true;
              if (current && result.status !== 'cancelled' && !recoverUnavailable)
                this.#failed(desired, result);
              continue;
            }
          } catch (error: unknown) {
            if (
              this.#current(binding, desired) &&
              displayEpoch === this.#preview?.epoch &&
              (!admitted || (this.#viewport === admitted && admitted.scope.current()))
            )
              throw error;
            this.options.report(error);
            continue;
          } finally {
            admitted?.complete(false);
          }
        }
        const previewStep = this.#preview?.next();
        if (previewStep) {
          try {
            const result = await this.#send(binding, previewStep.operation, previewStep.started);
            if (!this.#owns(binding)) return;
            previewStep.settled(result);
          } catch (error: unknown) {
            previewStep.failed(error);
          }
          this.#dirty = true;
          continue;
        }
        if (this.#current(binding, desired)) {
          this.#show(true);
          this.options.status({
            state:
              desired.shown ||
              this.#preview?.displayed ||
              (desired.previewable &&
                this.#preview?.observing &&
                binding.selectionRevision !== null)
                ? 'ready'
                : this.#preview?.preparing
                  ? 'loading'
                  : 'empty',
            message: '',
          });
        }
      } catch (error: unknown) {
        if (this.#current(binding, desired)) {
          this.#failedIntent();
          this.options.status({ state: 'error', message: 'Native canvas presentation failed.' });
        }
        this.options.report(error);
      }
    }
  }

  /** Delegate through the same binding record for every operation. */
  #send(
    binding: NativeCanvasBinding,
    operation: NativeCanvasOperation,
    assigned?: (revision: number) => void,
  ): Promise<CanvasSurfaceResult> {
    return sendNativeCanvasOperation(binding, this.options.address, operation, assigned);
  }

  /** Let a native display transition supersede any still-pending final-camera acknowledgement. */
  #observed(
    binding: NativeCanvasBinding,
    display: CanvasSurfacePresentationDisplay | null,
    presentation: object | null,
  ): void {
    if (!this.#owns(binding)) return;
    const recovery = this.#viewportRecovery;
    if (
      display &&
      this.#desired === null &&
      recovery?.binding === binding &&
      recovery.presentation === presentation
    ) {
      this.#desired = recovery.desired;
      this.#viewportRecovery = null;
    }
    if (binding.active?.type === 'surface.viewport.request') binding.active.cancellation.cancel();
    if (this.#viewport) {
      this.#cancelViewport(this.#viewport);
      this.#viewport.complete(false);
    }
    this.options.preview(display?.type === 'preview');
    this.options.viewport(display && 'viewport' in display ? display.viewport : null);
    if (display) {
      binding.viewportKey = this.#desired?.key ?? null;
      this.#show(binding.committed && this.#desired?.geometry?.visible === true);
      this.options.status({ state: display.type === 'empty' ? 'empty' : 'ready', message: '' });
    }
  }

  #owns(binding: NativeCanvasBinding): boolean {
    return (
      !this.#disposed &&
      this.#binding === binding &&
      (this.options.accepts() || this.options.acceptsPreparation())
    );
  }

  #current(binding: NativeCanvasBinding, desired: Desired): boolean {
    return this.#owns(binding) && this.#desired === desired;
  }

  #failed(desired: Desired, result: CanvasSurfaceResult): void {
    if (this.#desired !== desired) return;
    this.#failedIntent();
    // A staged sibling temporarily owns native mutation. Its release explicitly refreshes
    // the retained attachment; permit that refresh to reconcile previously rejected geometry.
    if (result.status === 'stale') this.#desired = null;
    const error = result.status === 'error';
    this.options.status({
      state: error ? 'error' : 'unavailable',
      message:
        result.message ||
        (error ? 'Native canvas presentation failed.' : 'The native canvas is unavailable.'),
    });
    if (error)
      this.options.report(new Error(result.message || 'Native canvas presentation failed.'));
  }

  /** Settle unsent intent when its required presentation fails, without retrying that presentation. */
  #failedIntent(): void {
    this.#dirty = false;
    if (!this.#preview?.displayed) this.options.viewport(null);
    if (this.#viewport && !this.#viewport.running) this.#viewport.complete(false);
  }

  #show(visible: boolean): void {
    if (visible === this.#visible) return;
    this.#visible = visible;
    this.options.visible(visible);
  }

  /** Observe the host's release acknowledgement before allowing another binding to prepare. */
  #retire(): Promise<void> {
    this.#viewportRecovery = null;
    this.#show(false);
    this.options.viewport(null);
    this.options.preview(false);
    this.#preview?.dispose();
    this.#preview = null;
    if (this.#viewport) this.#cancelViewport(this.#viewport);
    const binding = this.#binding;
    this.#binding = null;
    if (!binding) return this.#retirement ?? Promise.resolve();
    binding.active?.cancellation.cancel();
    if (!binding.attempted) return this.#retirement ?? Promise.resolve();
    const release = releaseNativeCanvasBinding(binding, this.options.address);
    const retirement = this.#retirement
      ? Promise.all([this.#retirement, release]).then(() => {})
      : release;
    this.#retirement = retirement;
    void retirement.then(() => {
      if (this.#retirement === retirement) this.#retirement = null;
      this.#start();
    }, this.options.report);
    return retirement;
  }
}
