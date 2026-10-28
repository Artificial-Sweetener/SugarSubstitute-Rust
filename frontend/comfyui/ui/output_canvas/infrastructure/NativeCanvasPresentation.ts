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

import { CanvasCancellation } from '../../core/embedded/CanvasRuntime.js';
import type { WorkflowOutputItem } from '../../execution/domain/WorkflowOutput.js';
import type { CanvasAttachmentAddress } from '../domain/CanvasIdentity.js';
import type { CanvasSurfaceGeometry } from '../domain/CanvasViewPort.js';
import type {
  CanvasSurfaceCommand,
  CanvasSurfaceHost,
  CanvasSurfaceResult,
} from '../../../../../integrations/contracts/CanvasSurface.js';

interface Desired {
  readonly empty: boolean;
  readonly geometry: CanvasSurfaceGeometry | null;
  readonly item: WorkflowOutputItem | null;
  readonly key: string;
}

interface Binding {
  readonly host: CanvasSurfaceHost;
  revision: number;
  prepared: boolean;
  committed: boolean;
  attempted: boolean;
  geometry: string | null;
  lastGeometry: CanvasSurfaceGeometry | null;
  selection: string | null;
  active: {
    readonly cancellation: CanvasCancellation;
    readonly dependsOnView: boolean;
  } | null;
}

export interface NativeCanvasPresentationOptions {
  readonly address: CanvasAttachmentAddress;
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
  report(error: unknown): void;
}

/** Describe native readiness without retaining a browser image or native handle. */
export interface NativeCanvasPresentationStatus {
  readonly state: 'empty' | 'loading' | 'ready' | 'unavailable' | 'error';
  readonly message: string;
}

/** Keep one in-flight operation and coalesce newer view or geometry changes. */
export class NativeCanvasPresentation {
  #binding: Binding | null = null;
  #desired: Desired | null = null;
  #running = false;
  #dirty = false;
  #visible = false;
  #disposed = false;
  #retirement: Promise<void> | null = null;

  /** Bind native presentation to the existing qualified view address. */
  constructor(private readonly options: NativeCanvasPresentationOptions) {}

  /** Reconcile only the selection and geometry already chosen by the view owners. */
  update(geometry: CanvasSurfaceGeometry | null, item: WorkflowOutputItem | null): void {
    if (this.#disposed) return;
    const preparing = this.options.acceptsPreparation();
    const host = this.options.accepts() || preparing ? this.options.host() : null;
    if (host !== (this.#binding?.host ?? null)) {
      void this.#retire();
      if (host) {
        this.#binding = {
          host,
          revision: 0,
          prepared: false,
          committed: false,
          attempted: false,
          geometry: null,
          lastGeometry: null,
          selection: null,
          active: null,
        };
      }
      this.#desired = null;
    }
    const eligible =
      geometry &&
      geometry.viewport.width > 0 &&
      geometry.viewport.height > 0 &&
      (preparing || (geometry.visible && geometry.clip.width > 0 && geometry.clip.height > 0)) &&
      item?.mediaKind === 'image'
        ? item
        : null;
    const key = JSON.stringify([
      geometry,
      item && [item.id, item.file, item.mediaKind],
      this.options.accepts(),
    ]);
    if (this.#desired?.key === key) return;
    this.#desired = { geometry, item: eligible, key, empty: item?.mediaKind !== 'image' };
    // A previous allocation can no longer be presented after the native window resizes.
    // Cancel its acknowledgement wait so the current geometry can reach the host.
    if (this.#binding?.active?.dependsOnView) this.#binding.active.cancellation.cancel();
    // The native owner retains acknowledged pixels while a same-allocation replacement loads.
    if (
      !eligible ||
      !this.#binding?.committed ||
      this.#binding.geometry !== JSON.stringify(geometry)
    )
      this.#show(false);
    this.options.status(
      eligible
        ? host
          ? { state: 'loading', message: 'Loading the selected image.' }
          : { state: 'unavailable', message: 'The native canvas is unavailable.' }
        : { state: 'empty', message: '' },
    );
    this.#dirty = true;
    this.#start();
  }

  /** Revoke the exact surface before releasing its transport and late completions. */
  dispose(): Promise<void> {
    if (this.#disposed) return this.#retirement ?? Promise.resolve();
    this.#disposed = true;
    this.#desired = null;
    return this.#retire();
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

  async #run(): Promise<void> {
    while (this.#dirty && !this.#disposed) {
      this.#dirty = false;
      const binding = this.#binding;
      const desired = this.#desired;
      if (!binding || !desired || !this.#owns(binding)) return;
      try {
        if (
          (desired.item || (this.options.acceptsPreparation() && desired.empty)) &&
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
        const geometry = desired.item
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
        if (!desired.item) {
          if (desired.empty) this.options.prepared();
          continue;
        }
        const selection = JSON.stringify([desired.item.id, desired.item.file]);
        if (selection !== binding.selection) {
          const result = await this.#send(binding, {
            type: 'surface.select',
            itemId: desired.item.id,
            file: desired.item.file,
          });
          if (!this.#owns(binding)) return;
          if (result.status !== 'applied') {
            this.#failed(desired, result);
            continue;
          }
          binding.selection = selection;
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
        if (this.#current(binding, desired)) {
          this.#show(true);
          this.options.status({ state: 'ready', message: '' });
        }
      } catch (error: unknown) {
        if (this.#current(binding, desired)) {
          this.options.status({ state: 'error', message: 'Native canvas presentation failed.' });
          this.options.report(error);
        }
      }
    }
  }

  /** Preserve one ordered revision stream for this attachment and host. */
  async #send(
    binding: Binding,
    operation:
      | { readonly type: 'surface.prepare' }
      | { readonly type: 'surface.commit' }
      | { readonly type: 'surface.geometry'; readonly geometry: CanvasSurfaceGeometry }
      | {
          readonly type: 'surface.select';
          readonly itemId: string;
          readonly file: WorkflowOutputItem['file'];
        },
  ): Promise<CanvasSurfaceResult> {
    const revision = this.#revision(binding);
    const cancellation = new CanvasCancellation();
    binding.active = { cancellation, dependsOnView: operation.type !== 'surface.prepare' };
    // Cancellation does not roll back an effect already accepted by the native owner.
    // Reconcile the next desired state even when it returns to the last acknowledged value.
    if (operation.type === 'surface.geometry') binding.geometry = null;
    if (operation.type === 'surface.select') binding.selection = null;
    binding.attempted = true;
    const command: CanvasSurfaceCommand = {
      ...operation,
      address: this.options.address,
      revision,
    };
    try {
      return await binding.host.execute(command, cancellation);
    } finally {
      if (binding.active?.cancellation === cancellation) binding.active = null;
    }
  }

  #revision(binding: Binding): number {
    if (binding.revision === Number.MAX_SAFE_INTEGER)
      throw new Error('Canvas surface revision exhausted.');
    return ++binding.revision;
  }

  #owns(binding: Binding): boolean {
    return (
      !this.#disposed &&
      this.#binding === binding &&
      (this.options.accepts() || this.options.acceptsPreparation())
    );
  }

  #current(binding: Binding, desired: Desired): boolean {
    return this.#owns(binding) && this.#desired === desired;
  }

  #failed(desired: Desired, result: CanvasSurfaceResult): void {
    if (this.#desired !== desired) return;
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

  #show(visible: boolean): void {
    if (visible === this.#visible) return;
    this.#visible = visible;
    this.options.visible(visible);
  }

  /** Observe the host's release acknowledgement before allowing another binding to prepare. */
  #retire(): Promise<void> {
    this.#show(false);
    const binding = this.#binding;
    this.#binding = null;
    if (!binding) return this.#retirement ?? Promise.resolve();
    binding.active?.cancellation.cancel();
    if (!binding.attempted) return this.#retirement ?? Promise.resolve();
    let release: Promise<void>;
    try {
      const revision = this.#revision(binding);
      release = binding.host
        .execute({ type: 'surface.release', address: this.options.address, revision })
        .then((result) => {
          if (result.status !== 'applied' && result.status !== 'stale')
            throw new Error(result.message || 'Native canvas release failed.');
        });
    } catch (error: unknown) {
      release = Promise.reject(
        error instanceof Error
          ? error
          : new Error('Native canvas release failed.', { cause: error }),
      );
    }
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
