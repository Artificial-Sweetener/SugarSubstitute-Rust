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
/** Pointer placement owns one immutable collision frame and finite current read/write/marker generations. */
import { requireValue } from '../../core/src/required.js';
import {
  currentReorderPointerCalibration,
  matchesReorderPointerIntent,
  requestReorderPointerCalibration,
  type ReorderPointerIntent,
} from './reorder-pointer-calibration.js';
import {
  readReorderLandingPaint,
  observeReorderLandingPaint,
  type ReorderLandingObservation,
} from './reorder-landing-paint-lease.js';
import type { EditorView } from '@codemirror/view';
import type { ReorderMeasureInput, ReorderMeasureOwner } from './reorder-measurement-contracts.js';
import { ReorderPointerFeedback } from './reorder-pointer-feedback.js';
import { ReorderCollisionBase } from './reorder-collision-base.js';
import { reorderPreview } from './reorder-preview.js';
import { type ReorderRect, type ReorderMeasuredGeometry } from './reorder-geometry.js';
import type { ReorderMotionOrigin } from './reorder-motion-origin.js';
/** The mounted resource owner supplies scheduling/failure observation and live proposed geometry. */
export interface ReorderPlacementPorts {
  /** An exact displayed whole endpoint supplies marker/landing paint, never collision geometry. */
  displayPaint(input: ReorderMeasureInput): ReorderLandingObservation | null;
  observe<T>(action: () => T): T;
  schedule<T>(request: {
    read(view: EditorView): T;
    write?(measured: T, view: EditorView): void;
    key?: unknown;
  }): void;
  markerRects(input: ReorderMeasureInput): readonly ReorderRect[];
  /** Display sampling stays distinct from the fixed collision operand. */
  /** READ may borrow an exact Presentation-issued no-proposal capability instead of sampling unused full geometry. */
  retain(input: ReorderMeasureInput): (() => boolean) | null;
  before(): ReorderMotionOrigin;
  propose(input: ReorderMeasureInput, origin: ReorderMotionOrigin): void;
  pending(): void;
  landing(
    input: ReorderMeasureInput,
    rects: readonly ReorderRect[],
    color: string,
    viewport: ReorderMeasuredGeometry['viewport'],
    pending: boolean,
  ): void;
}
/** A single hold's target lease is independent of the complete proposed paint and native capture owner. */
export class ReorderPointerPlacement {
  /** Read from the actual issued display owner; canonical admission remains exact when no whole endpoint owns pixels. */
  private readLanding<T>(
    input: ReorderMeasureInput,
    read: (current: () => boolean, preview: NonNullable<ReorderLandingObservation['preview']>) => T,
  ): T | null {
    const display = this.ports.displayPaint(input);
    if (display) {
      if (!display.current()) return null;
      const output = read(display.current, display.preview);
      return display.current() ? output : null;
    }
    return readReorderLandingPaint(
      this.view,
      this.view.state.field(reorderPreview, false)?.value ?? null,
      this.landingPaint,
      read,
    );
  }
  private readonly view: EditorView;
  private readonly owner: ReorderMeasureOwner;
  private readonly feedback: ReorderPointerFeedback;
  private readonly base: ReorderCollisionBase;
  private readonly ports: ReorderPlacementPorts;
  private epoch = 0;
  private markerGeneration = 0;
  private placement: ReorderPointerIntent | null = null;
  private landingPaint: ReorderLandingObservation | null = null;
  private disposed = false;
  /** Bind only actual mounted resources and the genuine session transformation callbacks. */
  constructor(
    view: EditorView,
    owner: ReorderMeasureOwner,
    feedback: ReorderPointerFeedback,
    ports: ReorderPlacementPorts,
  ) {
    this.view = view;
    this.owner = owner;
    this.feedback = feedback;
    this.ports = ports;
    this.base = new ReorderCollisionBase(view);
  }
  /** Preserve the mounted owner's exact external error boundary. */
  private observe<T>(action: () => T): T {
    return this.ports.observe(action);
  }
  /** Queue only supported native measurement jobs through the finite resource owner. */
  private schedule<T>(request: {
    read(view: EditorView): T;
    write?(measured: T, view: EditorView): void;
    key?: unknown;
  }): void {
    this.ports.schedule(request);
  }
  /** Collision reads never consume a foreign, restored, or already committed live presentation. */
  private published(input: ReorderMeasureInput): boolean {
    const preview = this.view.state.field(reorderPreview, false)?.value;
    return (
      preview?.token === input.token &&
      preview.capture === input.capture &&
      preview.order === input.order &&
      preview.presentation === 'pointer'
    );
  }
  /** Pure operation identity remains independent of delegated native context observations. */
  private ownsIntent(placement: ReorderPointerIntent): boolean {
    return (
      !this.disposed &&
      placement.epoch === this.epoch &&
      this.placement === placement &&
      this.published(placement.input)
    );
  }
  /** Bind the exact latest intent/retarget epoch while the real collision owner calibrates in separate native frames. */
  private measureBase(placement: ReorderPointerIntent): void {
    requestReorderPointerCalibration(placement, this.base, {
      owner: this.owner,
      owned: () => this.ownsIntent(placement),
      observe: (action) => this.observe(action),
      schedule: (request) => this.schedule(request),
      ready: () => {
        placement.calibration = null;
        this.motion(placement.input, placement.release);
      },
      refresh: () => this.refreshPlacement(placement.input, placement, 'stale'),
    });
  }
  /** Current physical pointer intents resolve only against the retained canonical base frame. */
  motion(input: ReorderMeasureInput, release: boolean): void {
    if (!input.hidden || !this.owner.current(input)) return;
    const pending = this.placement;
    if (
      pending?.input === input &&
      this.observe(() =>
        currentReorderPointerCalibration(pending, {
          owner: this.owner,
          owned: () => this.ownsIntent(pending),
        }),
      )
    ) {
      pending.release ||= release;
      return;
    }
    if (!this.observe(() => this.base.prepare(input))) {
      this.owner.failed();
      return;
    }
    const placement: ReorderPointerIntent = {
      input,
      epoch: this.epoch,
      hidden: input.hidden,
      held: input.held,
      capture: input.capture,
      pointer: input.pointer,
      calibration: null,
      release: release || (this.placement?.input === input && this.placement.release),
    };
    this.placement = placement;
    this.observe(() => this.feedback.pendingMarker());
    this.observe(() => this.ports.pending());
    const epoch = this.epoch;
    this.schedule({
      key: input,
      read: () =>
        this.observe(() => {
          if (
            this.placement !== placement ||
            !this.owner.current(input) ||
            !this.published(input) ||
            !input.hidden ||
            !input.coordinates
          )
            return null;
          const coordinates = input.coordinates,
            hidden = input.hidden,
            probe = this.feedback.probe(coordinates),
            context = this.base.contextAt(input),
            rects = context && this.base.current(input, context);
          const target =
            rects && probe
              ? this.base.target(
                  input,
                  requireValue(context, 'reorder-pointer-placement'),
                  probe.x,
                  probe.y,
                )
              : null;
          const order = input.order,
            held = input.held;
          const outcome = target ? this.owner.prepare(input, hidden, target, true) : null;
          if (
            !this.owner.current(input) ||
            this.placement !== placement ||
            input.order !== order ||
            input.hidden !== hidden ||
            input.coordinates !== coordinates ||
            input.held !== held
          )
            return null;
          const retained =
            target && !placement.release && outcome?.kind !== 'changed'
              ? this.ports.retain(input)
              : null;
          if (
            !this.owner.current(input) ||
            this.placement !== placement ||
            input.order !== order ||
            input.hidden !== hidden ||
            input.coordinates !== coordinates ||
            input.held !== held
          )
            return null;
          return {
            context,
            needsBase: !rects,
            rects,
            coordinates,
            hidden,
            order,
            held,
            outcome,
            retained,
            origin: target && !placement.release && !retained ? this.ports.before() : null,
            viewport: this.view.scrollDOM.getBoundingClientRect(),
            target,
          };
        }),
      write: (measured) => {
        if (
          !this.disposed &&
          epoch === this.epoch &&
          this.placement === placement &&
          measured &&
          this.owner.current(input) &&
          this.published(input) &&
          input.coordinates === measured.coordinates &&
          input.hidden === measured.hidden &&
          input.order === measured.order
        ) {
          this.observe(() => {
            if (measured.needsBase && measured.context) {
              placement.calibration = measured.context;
              this.base.mount(input, measured.context);
            }
          });
        }
        if (measured?.needsBase) {
          const context = measured.context;
          if (context) {
            if (placement.calibration === context && this.ownsIntent(placement))
              this.measureBase(placement);
          } else
            queueMicrotask(() =>
              this.observe(() => {
                if (
                  this.ownsIntent(placement) &&
                  this.owner.current(input) &&
                  this.ownsIntent(placement)
                )
                  this.owner.failed();
              }),
            );
          return;
        }
        queueMicrotask(() => {
          if (
            this.disposed ||
            epoch !== this.epoch ||
            this.placement !== placement ||
            !this.owner.current(input) ||
            !this.published(input) ||
            !measured ||
            input.coordinates !== measured.coordinates ||
            input.hidden !== measured.hidden ||
            input.order !== measured.order
          )
            return;
          this.observe(() => {
            if (measured.context && this.base.current(input, measured.context) !== measured.rects) {
              this.motion(input, placement.release);
              return;
            }
            if (
              (measured.outcome &&
                measured.outcome.kind !== 'unavailable' &&
                !measured.outcome.current()) ||
              (measured.retained && !measured.retained())
            ) {
              if (
                epoch === this.epoch &&
                this.placement === placement &&
                this.owner.current(input) &&
                epoch === this.epoch &&
                this.placement === placement
              )
                this.motion(input, placement.release);
              return;
            }
            if (
              this.disposed ||
              epoch !== this.epoch ||
              this.placement !== placement ||
              input.coordinates !== measured.coordinates ||
              input.order !== measured.order ||
              input.hidden !== measured.hidden ||
              input.held !== measured.held ||
              !this.owner.current(input) ||
              this.disposed ||
              epoch !== this.epoch ||
              this.placement !== placement ||
              input.coordinates !== measured.coordinates ||
              input.order !== measured.order ||
              input.hidden !== measured.hidden ||
              input.held !== measured.held
            )
              return;
            this.placement = null;
            if (measured.outcome?.kind === 'changed') measured.outcome.apply();
            if (
              this.disposed ||
              epoch !== this.epoch ||
              !this.owner.current(input) ||
              !this.published(input)
            )
              return;
            if (input.order !== measured.order) {
              ++this.markerGeneration;
              this.landingPaint = null;
              this.feedback.clearMarker();
              this.ports.pending();
            }
            if (
              measured.target &&
              measured.origin &&
              !placement.release &&
              this.owner.current(input)
            )
              this.ports.propose(input, measured.origin);
            if (placement.release) this.owner.commit(input);
            else if (
              measured.target &&
              this.owner.current(input) &&
              input.coordinates === measured.coordinates &&
              [
                measured.viewport.left,
                measured.viewport.right,
                measured.viewport.top,
                measured.viewport.bottom,
              ].every(Number.isFinite)
            )
              this.mark(input);
          });
        });
      },
    });
  }
  /** Actual geometry supersedes pending placement, preserving release; stationary feedback never transforms the order. */
  invalidateGeometry(input: ReorderMeasureInput): void {
    ++this.markerGeneration;
    this.landingPaint = null;
    if (this.placement?.input === input) this.placement = { ...this.placement, calibration: null };
    this.feedback.clearMarker();
  }
  /** Renew current placement and marker only after CM's enclosing update unlocks. */
  refresh(input: ReorderMeasureInput): void {
    this.refreshPlacement(input, this.placement, 'geometry');
  }
  /** A newly admitted complete display renews only marker READ; the immutable collision calibration remains untouched. */
  endpoint(input: ReorderMeasureInput): void {
    if (!this.disposed && this.owner.current(input) && this.published(input)) this.mark(input);
  }
  /** Every delegated frontier retains the exact refreshed intent; stale calibration cannot renew a released or newer hold. */
  private refreshPlacement(
    input: ReorderMeasureInput,
    placement: ReorderPointerIntent | null,
    cause: 'geometry' | 'stale',
  ): void {
    /** The stale phase borrows immutable collision operands across currentness and marker callbacks. */
    const owns = (): boolean =>
      placement
        ? placement.input === input &&
          matchesReorderPointerIntent(placement) &&
          this.ownsIntent(placement)
        : !this.disposed && this.placement === null;
    /** Source and release admission cannot survive callback transfer to a newer phase. */
    const current = (): boolean => {
      if (!owns()) return false;
      const retained = this.owner.current(input);
      if (!owns()) return false;
      const released = cause === 'stale' && placement?.release;
      if (cause === 'stale' && (!retained || released)) this.owner.failed();
      return retained && !released;
    };
    if (!current() || input.pointer === null || !input.hidden) return;
    ++this.markerGeneration;
    this.landingPaint = null;
    this.observe(() => this.feedback.clearMarker());
    if (!current()) return;
    if (placement?.input === input) {
      placement.calibration = null;
      this.motion(input, placement.release);
    } else this.mark(input);
  }
  /** Remeasure the actual invisible held destination in the complete live proposal. */
  private mark(input: ReorderMeasureInput): void {
    const epoch = this.epoch,
      generation = ++this.markerGeneration,
      order = input.order,
      hidden = input.hidden,
      held = input.held,
      coordinates = input.coordinates;
    this.schedule({
      key: this.feedback,
      read: () =>
        this.observe(() =>
          this.disposed ||
          epoch !== this.epoch ||
          generation !== this.markerGeneration ||
          !hidden ||
          input.order !== order ||
          !this.published(input) ||
          !this.owner.current(input) ||
          input.hidden !== hidden ||
          input.order !== order ||
          input.held !== held
            ? null
            : this.readLanding(input, (lease, preview) => {
                const measured = observeReorderLandingPaint(
                  this.view,
                  input,
                  preview,
                  lease,
                  this.landingPaint,
                  this.ports.markerRects(input),
                  this.ports.displayPaint(input),
                );
                if (lease()) this.landingPaint = measured?.observation ?? null;
                return measured;
              }),
        ),
      write: (measured) => {
        if (
          !this.disposed &&
          epoch === this.epoch &&
          generation === this.markerGeneration &&
          this.view.state.field(reorderPreview, false)?.value?.order === order &&
          this.view.state.field(reorderPreview, false)?.value?.capture === input.capture &&
          this.owner.current(input) &&
          this.published(input) &&
          input.hidden === hidden &&
          input.order === order &&
          input.held === held &&
          (!measured || measured.lease())
        )
          this.observe(() => {
            const pending = this.placement !== null || input.coordinates !== coordinates;
            this.feedback.mark(measured?.rect ?? null, pending);
            if (measured)
              this.ports.landing(
                input,
                measured.landingRects,
                measured.color,
                measured.viewport,
                pending,
              );
          });
      },
    });
  }
  /** Cancel every pending placement/marker phase and release the actual passive scene. */
  cancel(): void {
    ++this.epoch;
    ++this.markerGeneration;
    this.placement = null;
    this.landingPaint = null;
    this.base.destroy();
  }
  /** Teardown prevents any retained native callback from renewing a collision lease. */
  destroy(): void {
    this.disposed = true;
    this.cancel();
  }
}
