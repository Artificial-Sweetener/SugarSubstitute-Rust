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
/** A finite passive display owner samples actual CM paint once per proposal and writes only immutable numeric frames. */
import { requireValue } from '../../core/src/required.js';
import type { EditorView } from '@codemirror/view';
import type { ReorderMotionPorts } from './reorder-motion-contracts.js';
import type { ReorderRect, ReorderMeasuredGeometry } from './reorder-geometry.js';
import type { ReorderMeasureInput } from './reorder-measurement-contracts.js';
import { reorderPreview, fallbackReorderPointerPaint } from './reorder-preview.js';
import type { ReorderPreview } from './reorder-preview-contracts.js';
import { ReorderMotionClock, type ReorderMotionFrame } from './reorder-motion-clock.js';
import { reorderMotionTargets } from './reorder-motion-targets.js';
import { ReorderMotionPreparation } from './reorder-motion-preparation.js';
import type { ReorderMotionOrigin } from './reorder-motion-origin.js';
import { ReorderMotionResource } from './reorder-motion-resource.js';
import { ReorderMotionLanding } from './reorder-motion-landing.js';
import {
  readDisplayedReorderOrigin,
  captureDisplayedReorderGrab,
  type ReorderPausedDisplayGrab,
  type ReorderCapturedDisplayGrab,
  readCurrentDisplayedReorderPaint,
  hitCurrentDisplayedReorderPaint,
  borrowDisplayedReorderGrab,
  retainedDisplayedReorderScene,
  type ReorderAcceptedDisplayPaint,
  type ReorderDisplayedPaintPorts,
} from './reorder-displayed-paint.js';
import type { ReorderLandingObservation } from './reorder-landing-paint-lease.js';
/** One accepted native paint stamp owns capture, supported suppression, timing and landing retirement together. */
export class ReorderMotionPresentation {
  private readonly view: EditorView;
  private readonly ports: ReorderMotionPorts;
  private readonly clock: ReorderMotionClock;
  private readonly landing: ReorderMotionLanding;
  private readonly media: MediaQueryList | null;
  private generation = 0;
  private readonly resource: ReorderMotionResource;
  private readonly display: ReorderDisplayedPaintPorts;
  private acceptedPaint: ReorderAcceptedDisplayPaint | null = null;
  private input: ReorderMeasureInput | null = null;
  private readonly preparation: ReorderMotionPreparation;
  private frame: ReorderMotionFrame | null = null;
  private preparingPaint: {
    readonly input: ReorderMeasureInput;
    readonly preview: ReorderPreview;
    readonly held: number;
    readonly pointer: number | null;
  } | null = null;
  private disposed = false;
  private grab: ReorderPausedDisplayGrab | null = null;
  /** Bind only the existing native measure/currentness boundary; no domain or request owner is introduced. */
  constructor(view: EditorView, ports: ReorderMotionPorts) {
    this.view = view;
    this.ports = ports;
    this.preparation = new ReorderMotionPreparation(view, ports);
    this.resource = new ReorderMotionResource(view, {
      observe: (action) => ports.observe(action),
    });
    this.display = {
      live: () => !this.disposed,
      scene: () => this.resource.scene,
      frame: () => this.frame,
      input: () => this.input,
      accepted: () => this.acceptedPaint,
      current: (input) => this.ports.current(input),
      resourceCurrent: () => this.resource.current(),
    };
    const window = requireValue(view.dom.ownerDocument.defaultView, 'reorder-motion-presentation');
    this.media = window.matchMedia?.('(prefers-reduced-motion: reduce)') ?? null;
    this.landing = new ReorderMotionLanding(
      view.dom.ownerDocument,
      () => !this.disposed,
      (input) => ports.current(input),
      () => this.preparingPaint !== null,
    );
    this.clock = new ReorderMotionClock({
      now: () => window.performance.now(),
      request: (callback) => window.requestAnimationFrame(callback),
      cancel: (handle) => window.cancelAnimationFrame(handle),
      current: () => {
        const current =
          !this.disposed && !!this.input && ports.current(this.input) && this.resource.current();
        if (!current && this.resource.scene) this.releasePaint('scene');
        return current;
      },
      paint: (frame) => {
        this.frame = frame;
        this.resource.scene?.paint(frame);
      },
      settled: () => {
        if (!this.resource.scene?.whole) this.releasePaint('scene');
      },
      failed: (cause) =>
        ports.observe(() => {
          throw cause;
        }),
    });
    try {
      this.media?.addEventListener('change', this.motionChanged);
      view.dom.ownerDocument.fonts?.addEventListener('loadingdone', this.motionChanged);
    } catch (cause) {
      try {
        this.destroy();
      } catch (cleanup) {
        throw new AggregateError([cause, cleanup], 'reorder-motion-construction-failed');
      }
      throw cause;
    }
  }
  /** Policy changes retire animated suppression immediately and reveal the complete canonical native target. */
  private readonly motionChanged = (): void => {
    this.ports.observe(() => {
      const input = this.input ?? this.landing.input;
      this.cancel();
      if (input && this.ports.current(input)) this.ports.refresh(input, 'geometry');
    });
  };
  /** READ borrows exact display geometry; proposal and clock authority remain here. */
  before(): ReorderMotionOrigin {
    return readDisplayedReorderOrigin(this.view, this.display);
  }
  /** The current whole endpoint owns marker paint independently of its frozen CM pickup projection. */
  displayPaint(input: ReorderMeasureInput): ReorderLandingObservation | null {
    return readCurrentDisplayedReorderPaint(this.view, this.display, input);
  }
  /** A genuine press resolves actual displayed identity; semantic membership remains with Session. */
  hit(x: number, y: number): number | null {
    return hitCurrentDisplayedReorderPaint(this.view, this.display, x, y);
  }
  /** Pause the exact display issue before a primary press changes semantic held identity. */
  reserveGrab(
    held: number,
    press: {
      readonly x: number;
      readonly y: number;
    },
  ): boolean {
    const display = borrowDisplayedReorderGrab(this.view, this.display, held, press);
    if (!display) return false;
    this.clock.cancel();
    if (!display.current()) return false;
    this.grab = { held, geometry: display.geometry, scene: display.scene };
    return true;
  }
  /** Copy the paused accepted viewport paint once in CM's read phase, preserving the actual press hotspot and native glyph/media pixels. */
  captureGrab(held: number): ReorderCapturedDisplayGrab | null {
    const grab = this.grab;
    this.grab = null;
    return captureDisplayedReorderGrab(this.view, this.display, held, grab);
  }
  /** A pending read dims the current issued contour; accepted warm geometry retains its exact DOM lease. */
  pending(): void {
    this.settleLanding(true);
  }
  /** READ issues reuse only for this exact accepted stamp or pending request; WRITE may consume it without sampling new geometry. */
  retain(input: ReorderMeasureInput): (() => boolean) | null {
    const preview = this.view.state.field(reorderPreview, false)?.value,
      generation = this.generation,
      held = input.held,
      pointer = input.pointer;
    if (!preview) return null;
    /** Delegated currentness cannot authorize a replaced native issue, hold or publication. */
    const current = (): boolean =>
      !this.disposed &&
      generation === this.generation &&
      input.held === held &&
      input.pointer === pointer &&
      this.view.state.field(reorderPreview, false)?.value === preview &&
      this.ports.current(input) &&
      !this.disposed &&
      generation === this.generation &&
      input.held === held &&
      input.pointer === pointer &&
      this.view.state.field(reorderPreview, false)?.value === preview;
    const accepted = this.acceptedPaint;
    if (accepted?.input === input && accepted.preview === preview) {
      /** Current unavailability is intentional deduplication, exactly like current complete geometry. */
      const admitted = (): boolean =>
        this.acceptedPaint === accepted &&
        current() &&
        accepted.current() &&
        this.acceptedPaint === accepted &&
        current();
      if (admitted())
        return () => {
          if (!admitted()) return false;
          this.preparingPaint = null;
          return true;
        };
    }
    const pending = this.preparingPaint;
    if (
      pending?.input !== input ||
      pending.preview !== preview ||
      pending.held !== held ||
      pending.pointer !== pointer
    )
      return null;
    const lease = this.preparation.pendingCurrent();
    if (!lease) return null;
    /** A later successful request cannot replace the exact request/child captured by this read. */
    const admitted = (): boolean =>
      this.preparingPaint === pending &&
      current() &&
      lease() &&
      this.preparingPaint === pending &&
      current();
    return admitted() ? admitted : null;
  }
  /** The accepted Core publication is measured after native layout; capture failure falls back to immediate complete native paint. */
  propose(input: ReorderMeasureInput, origin: ReorderMotionOrigin): void {
    const preview = this.view.state.field(reorderPreview, false)?.value,
      held = input.held,
      primary = input.pointer;
    if (!preview || !this.ports.current(input)) return;
    if (this.retain(input)?.()) return;
    const generation = ++this.generation;
    this.preparingPaint = null;
    this.preparation.retirePending();
    if (
      generation !== this.generation ||
      this.disposed ||
      !this.ports.current(input) ||
      this.view.state.field(reorderPreview, false)?.value !== preview
    )
      return;
    this.input = input;
    this.preparingPaint = {
      input,
      preview,
      held: input.held,
      pointer: input.pointer,
    };
    const current = (): boolean =>
      generation === this.generation &&
      !this.disposed &&
      this.ports.current(input) &&
      generation === this.generation &&
      !this.disposed &&
      this.view.state.field(reorderPreview, false)?.value === preview;
    const pointer = this.view.state.field(reorderPreview).pointer;
    const whole = pointer?.mode === 'stage';
    /** Only this exact issued proposal can latch its own primary hold to native fallback. */
    const fallback = (): void => {
      if (whole && current()) fallbackReorderPointerPaint(this.view, pointer, preview, current);
    };
    if (this.media?.matches && !whole) {
      this.preparingPaint = null;
      this.releasePaint('scene');
      return;
    }
    this.preparation.prepare(input, {
      current,
      select: (geometry) =>
        reorderMotionTargets(origin, geometry, input.pointer === null ? input.held : null),
      pending: () => this.settleLanding(),
      fallback,
      replaceContext: () => this.releasePaint('context'),
      unavailable: (lease) => {
        if (!current()) return;
        this.preparingPaint = null;
        this.settleLanding();
        if (!current()) return;
        const accepted = lease?.();
        if (!current()) return;
        if (accepted && lease)
          this.acceptedPaint = {
            preview,
            input,
            current: lease,
            rects: null,
          };
        this.releasePaint('scene');
        if (!whole || !pointer || !current()) return;
        const publication = this.view.state.field(reorderPreview, false);
        // Canonical fallback owns the next marker READ only after this exact Stage has retired.
        if (
          publication?.value === preview &&
          publication.pointer?.hold === pointer.hold &&
          publication.pointer.mode === 'fallback' &&
          current()
        )
          this.ports.refresh(input, 'endpoint');
      },
      ready: (scene, lease, retained) => {
        this.preparingPaint = null;
        this.settleLanding();
        const targets = scene.targets;
        if (!targets.length && !scene.whole) {
          this.acceptedPaint = {
            preview,
            input,
            current: requireValue(lease, 'reorder-motion-presentation'),
            rects: scene.geometry,
          };
          scene.destroy();
          this.releasePaint('scene');
          return;
        }
        const oldScene = this.resource.scene;
        try {
          this.clock.cancel();
        } catch (primary) {
          const failures: unknown[] = [primary];
          for (const release of [
            fallback,
            () => {
              if (generation === this.generation && this.resource.scene === oldScene) {
                this.input = null;
                this.resource.release();
              }
            },
            () => {
              if (this.resource.scene !== scene) scene.destroy();
            },
          ])
            try {
              release();
            } catch (cleanup) {
              failures.push(cleanup);
            }
          if (failures.length > 1)
            throw new AggregateError(failures, 'reorder-motion-clock-admission-failed');
          throw primary;
        }
        if (!current()) {
          scene.destroy();
          return;
        }
        this.input = input;
        // Resource owns physical lease admission; this predicate supplies caller lifetime.
        const admitted = () => !this.disposed;
        if (
          !this.resource.adopt(
            scene,
            retained,
            input.capture,
            scene.whole ? 'whole' : input.pointer === null ? 'native' : 'held',
            admitted,
            this.motionChanged,
            current,
            () => this.releasePaint('scene'),
            scene.whole
              ? () =>
                  this.landing.prepareEndpoint(
                    input,
                    scene,
                    pointer?.hold,
                    () => this.resource.scene === scene && this.resource.current(),
                  )
              : null,
          )
        ) {
          fallback();
          return;
        }
        if (current()) {
          this.acceptedPaint = {
            preview,
            input,
            current: requireValue(lease, 'reorder-motion-presentation'),
            rects: scene.geometry,
          };
          if (scene.whole) this.ports.refresh(input, 'endpoint');
        }
        if (
          current() ||
          retainedDisplayedReorderScene(this.view, this.display, {
            scene,
            pointer,
            preview,
            held,
            primary,
            retained,
          })
        )
          this.clock.replace(targets, !!this.media?.matches || targets.length === 0);
      },
    });
  }
  /** Actual native held fragments publish through the one landing contour lifetime. */
  mark(
    input: ReorderMeasureInput,
    rects: readonly ReorderRect[],
    color: string,
    viewport: ReorderMeasuredGeometry['viewport'],
    pointerPending = false,
  ): void {
    this.landing.show(input, rects, color, viewport, pointerPending);
  }
  /** Pending geometry dims the exact retained contour without another native read. */
  private settleLanding(pointerPending = false): void {
    this.landing.pending(pointerPending);
  }
  /** Every owned release is attempted; original ordered causes remain observable. */
  private releasePaint(scope: 'scene' | 'context'): void {
    const generation = this.generation;
    const scene = this.resource.scene;
    const publication = scene?.whole ? this.view.state.field(reorderPreview, false) : null;
    const pointer = publication?.pointer,
      preview = publication?.value;
    this.input = null;
    const failures: unknown[] = [];
    for (const release of [
      () => {
        if (scene?.whole && pointer?.mode === 'stage' && preview)
          fallbackReorderPointerPaint(
            this.view,
            pointer,
            preview,
            () =>
              generation === this.generation &&
              this.resource.scene === scene &&
              this.view.state.field(reorderPreview, false)?.value === preview,
          );
      },
      () => {
        if (generation === this.generation && this.resource.scene === scene) this.clock.cancel();
      },
      () => {
        if (generation === this.generation && this.resource.scene === scene) {
          if (scope === 'context') this.resource.retire();
          else this.resource.release();
        }
      },
    ])
      try {
        release();
      } catch (cause) {
        failures.push(cause);
      }
    if (failures.length === 1) throw failures[0];
    if (failures.length) throw new AggregateError(failures, 'reorder-motion-retirement-failed');
  }
  /** Revoke prepared geometry and the child landing operand before deferred retirement releases retained paint. */
  invalidateGeometry(): void {
    ++this.generation;
    this.input = null;
    this.acceptedPaint = null;
    this.preparingPaint = null;
    this.grab = null;
    this.ports.observe(() => this.landing.clear());
  }
  /** Deferred retirement releases the retained resources after synchronous publication invalidation. */
  cancel(): void {
    const generation = ++this.generation;
    const failures: unknown[] = [];
    for (const release of [
      () => this.releasePaint('context'),
      () => this.preparation.cancel(),
      () => this.clock.reset(),
      () => this.landing.clear(),
    ])
      try {
        if (generation === this.generation) release();
      } catch (cause) {
        failures.push(cause);
      }
    if (generation === this.generation) {
      this.frame = null;
      this.acceptedPaint = null;
      this.preparingPaint = null;
      this.grab = null;
    }
    if (failures.length === 1) throw failures[0];
    if (failures.length) throw new AggregateError(failures, 'reorder-motion-cancellation-failed');
  }
  /** No native policy callback or frame may renew a disposed display resource. */
  destroy(): void {
    this.disposed = true;
    const failures: unknown[] = [];
    for (const release of [
      () => this.media?.removeEventListener('change', this.motionChanged),
      () =>
        this.view.dom.ownerDocument.fonts?.removeEventListener('loadingdone', this.motionChanged),
      () => this.cancel(),
      () => this.clock.destroy(),
    ])
      try {
        release();
      } catch (cause) {
        failures.push(cause);
      }
    if (failures.length === 1) throw failures[0];
    if (failures.length) throw new AggregateError(failures, 'reorder-motion-disposal-failed');
  }
}
