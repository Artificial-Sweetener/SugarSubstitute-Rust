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
/** Finite canonical preparation owns proposal mounts and native calibration scheduling, independently of motion/landing publication. */
import type { EditorView } from '@codemirror/view';
import type { ReorderMeasureInput } from './reorder-measurement-contracts.js';
import type { ReorderMotionPorts } from './reorder-motion-contracts.js';
import type { ReorderMotionScene, ReorderMotionSelection } from './reorder-motion-scene.js';
import { reorderPreview } from './reorder-preview.js';
import { currentReorderProjectionLease } from './reorder-paint-projection.js';
import { PendingPassiveMotion } from './reorder-motion-passive.js';
import { preparePassiveMotion } from './reorder-passive-context.js';
import { ReorderPassiveStage } from './reorder-motion-stage.js';
/** Presentation accepts only actual complete output; rejected or unavailable output retains native paint. */
export interface PassivePreparationCallbacks {
  current(): boolean;
  /** Select only actual translated peers from complete geometry during native READ, before media/reveal. */
  select: ReorderMotionSelection;
  pending(): void;
  /** Presentation alone publishes the exact current canonical fallback before paint retirement. */
  fallback(): void;
  replaceContext(): void;
  unavailable(lease: (() => boolean) | null): void;
  ready(scene: ReorderMotionScene, lease: () => boolean, retained: () => boolean): void;
}
/** A read-issued freshness stamp belongs to only this finite preparation, not to a later same-preview request. */
interface PendingPreparation {
  current(): boolean;
}
/** One gesture plane survives readonly order issues; each pending child has one exact retirement owner. */
export class ReorderMotionPreparation {
  private readonly view: EditorView;
  private readonly ports: ReorderMotionPorts;
  private stage: ReorderPassiveStage | null = null;
  private pending: PendingPassiveMotion | null = null;
  private request: PendingPreparation | null = null;
  /** Bind the existing public CM measurement/observation transport. */
  constructor(view: EditorView, ports: ReorderMotionPorts) {
    this.view = view;
    this.ports = ports;
  }
  /** Retire the exact old proposal before invoking any external resource release. */
  retirePending(): void {
    const pending = this.pending;
    this.pending = null;
    this.request = null;
    pending?.destroy();
  }
  /** Same-publication delivery borrows only a still-current request and its captured native context; no new geometry/style read occurs here. */
  pendingCurrent(): (() => boolean) | null {
    const request = this.request,
      pending = this.pending;
    if (!request) return null;
    /** Exact request and child identities fence every delegated native-context check and completion. */
    const current = (): boolean =>
      this.request === request &&
      this.pending === pending &&
      request.current() &&
      this.request === request &&
      this.pending === pending &&
      (!pending || pending.current()) &&
      this.request === request &&
      this.pending === pending;
    return current() ? current : null;
  }
  /** Retire only the exact completed request before external callbacks can issue a newer one. */
  private finish(request: PendingPreparation): void {
    if (this.request === request) this.request = null;
  }
  /** Publish current fallback, retire only its exact Stage/extent, then permit scene/gate release. */
  private rejectStage(
    stage: ReorderPassiveStage,
    retire: () => void,
    callbacks: PassivePreparationCallbacks,
    lease: (() => boolean) | null,
  ): void {
    const owned = this.stage === stage;
    if (owned) this.stage = null;
    const failures: unknown[] = [];
    let fallbackPublished = false;
    try {
      if (owned && callbacks.current()) {
        callbacks.fallback();
        fallbackPublished = true;
      }
    } catch (cause) {
      failures.push(cause);
    }
    for (const release of [retire, ...(owned ? [() => stage.destroy()] : [])]) {
      try {
        release();
      } catch (cause) {
        failures.push(cause);
      }
    }
    try {
      if (fallbackPublished && this.stage === null && callbacks.current())
        callbacks.unavailable(lease);
    } catch (cause) {
      failures.push(cause);
    }
    if (failures.length === 1) throw failures[0];
    if (failures.length)
      throw new AggregateError(failures, 'reorder-motion-admission-retirement-failed');
  }
  /** Native READ/WRITE phases admit one current canonical child; warm requests reuse actual calibrated root output. */
  prepare(input: ReorderMeasureInput, callbacks: PassivePreparationCallbacks): void {
    const preview = this.view.state.field(reorderPreview, false)?.value;
    if (!preview || !callbacks.current()) return;
    const request: PendingPreparation = { current: callbacks.current };
    this.request = request;
    this.ports.schedule({
      key: this,
      read: () =>
        this.ports.observe(() => {
          if (!callbacks.current()) return null;
          const lease = currentReorderProjectionLease(this.view, preview);
          const prepared = lease
            ? preparePassiveMotion(this.view, input.pointer === null ? null : input.held)
            : null;
          const stage = this.stage;
          request.current = () =>
            callbacks.current() && !!lease?.() && !!prepared?.context.current();
          return {
            lease,
            prepared,
            stage,
            placement: prepared && stage ? stage.readPlacement(prepared) : null,
          };
        }),
      write: (measured) =>
        queueMicrotask(() =>
          this.ports.observe(() => {
            if (!callbacks.current()) return;
            callbacks.pending();
            if (!callbacks.current()) return;
            const lease = measured?.lease ?? null,
              prepared = measured?.prepared;
            if (!prepared || !lease?.()) {
              if (measured?.stage)
                this.rejectStage(measured.stage, () => this.finish(request), callbacks, lease);
              else {
                this.finish(request);
                callbacks.fallback();
                callbacks.unavailable(lease);
              }
              return;
            }
            if (this.stage && !this.stage.matches(prepared)) {
              const previous = this.stage;
              this.stage = null;
              const failures: unknown[] = [];
              for (const release of [callbacks.replaceContext, () => previous.destroy()]) {
                try {
                  release();
                } catch (cause) {
                  failures.push(cause);
                }
              }
              if (failures.length === 1) throw failures[0];
              if (failures.length)
                throw new AggregateError(failures, 'Motion context replacement failed');
              if (!callbacks.current()) return;
            }
            const previous = this.stage,
              stage = previous ?? new ReorderPassiveStage(this.view, prepared);
            if (!callbacks.current()) {
              if (stage !== previous) stage.destroy();
              return;
            }
            this.stage = stage;
            if (measured?.stage === stage && measured.placement) {
              stage.place(prepared, measured.placement);
              if (!callbacks.current() || this.stage !== stage) return;
            }
            const pending = stage.mount(prepared);
            if (!pending) {
              this.rejectStage(stage, () => this.finish(request), callbacks, lease);
              return;
            }
            if (!callbacks.current() || this.stage !== stage) {
              pending.destroy();
              return;
            }
            this.pending = pending;
            /** Exact-child retirement cannot clear a newer reentrant proposal. */
            const retire = (): void => {
              this.finish(request);
              if (this.pending === pending) this.pending = null;
              pending.destroy();
            };
            /** The first native result calibrates placement; subsequent proposals reuse that exact actual result. */
            const measure = (): void =>
              this.ports.schedule({
                key: this,
                read: () =>
                  this.ports.observe(() =>
                    callbacks.current() && lease() ? pending.read(callbacks.select) : null,
                  ),
                write: (read) =>
                  queueMicrotask(() =>
                    this.ports.observe(() => {
                      if (!callbacks.current() || !lease()) {
                        retire();
                        return;
                      }
                      if (!read || read.kind === 'unavailable') {
                        this.rejectStage(stage, retire, callbacks, lease);
                        return;
                      }
                      if (read.kind === 'layer') {
                        if (callbacks.current()) measure();
                        else retire();
                        return;
                      }
                      if (read.kind === 'placement') {
                        pending.place(read);
                        if (callbacks.current()) measure();
                        else retire();
                        return;
                      }
                      if (read.kind === 'media') {
                        if (pending.hydrate(read.plan) && callbacks.current()) measure();
                        else {
                          this.rejectStage(stage, retire, callbacks, lease);
                        }
                        return;
                      }
                      if (read.kind === 'carriers') {
                        if (pending.configureCarriers(read.operation) && callbacks.current())
                          measure();
                        else this.rejectStage(stage, retire, callbacks, lease);
                        return;
                      }
                      if (!read.scene.whole) pending.reveal();
                      this.finish(request);
                      if (this.pending === pending) this.pending = null;
                      if (!callbacks.current() || !lease()) {
                        pending.destroy();
                        return;
                      }
                      callbacks.ready(
                        read.scene,
                        lease,
                        () => stage.current() && pending.retained(),
                      );
                    }),
                  ),
              });
            measure();
          }),
        ),
    });
  }
  /** Retire children and coordinate plane independently, preserving all ordered raw cleanup failures. */
  cancel(): void {
    const stage = this.stage;
    this.stage = null;
    const failures: unknown[] = [];
    for (const release of [() => this.retirePending(), () => stage?.destroy()]) {
      try {
        release();
      } catch (cause) {
        failures.push(cause);
      }
    }
    if (failures.length === 1) throw failures[0];
    if (failures.length) throw new AggregateError(failures, 'Motion preparation retirement failed');
  }
}
