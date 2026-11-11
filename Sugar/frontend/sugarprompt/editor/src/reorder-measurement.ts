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
/** One mounted resource owns native measure phases, external proxy, pointer capture and finite edge scrolling. */
import { requireValue } from '../../core/src/required.js';
import {
  requestReorderPointerAdmission,
  type ReorderPointerPress,
} from './reorder-pointer-admission.js';
import type { EditorState } from '@codemirror/state';
import type { EditorView } from '@codemirror/view';
import { ReorderPointerFeedback } from './reorder-pointer-feedback.js';
import { ReorderPointerPlacement } from './reorder-pointer-placement.js';
import { ReorderKeyboardMeasurement } from './reorder-keyboard-measurement.js';
import { captureReorderGrabPaint } from './reorder-grab-paint.js';
import type { ReorderGrabPaint } from './reorder-grab-contracts.js';
import type { ReorderMeasureInput, ReorderMeasureOwner } from './reorder-measurement-contracts.js';
import { readCurrentReorderGeometry } from './reorder-current-geometry.js';
import { ReorderMotionPresentation } from './reorder-motion-presentation.js';
/** Pinned donor Qt6.11.2 QApplication.startDragDistance(): ten logical pixels, using Manhattan pointer displacement. */
export const reorderDragDistance = 10;
/** Measurement retains only one mounted resource lifetime and current native intents. */
export class ReorderMeasurement {
  private readonly view: EditorView;
  private readonly owner: ReorderMeasureOwner;
  private epoch = 0;
  private readonly placement: ReorderPointerPlacement;
  private frame: number | null = null;
  private lastScroll = 0;
  /** Explicit native wheel retains only the current pointer position until real motion or a new hold. */
  private scrollPause: {
    readonly input: ReorderMeasureInput;
    readonly x: number;
    readonly y: number;
  } | null = null;
  private scrollGeneration = 0;
  private readonly feedback: ReorderPointerFeedback;
  private readonly presentation: ReorderMotionPresentation;
  private pointer: number | null = null;
  private press: ReorderPointerPress | null = null;
  private disposed = false;
  private measureDepth = 0;
  /** Keyboard target phases have one distinct finite owner, retired with the mounted resources. */
  readonly keyboard: ReorderKeyboardMeasurement;
  /** The actual CodeMirror view supplies supported read/write scheduling and native pointer capture. */
  constructor(view: EditorView, owner: ReorderMeasureOwner) {
    this.view = view;
    this.owner = owner;
    this.feedback = new ReorderPointerFeedback(view);
    this.presentation = new ReorderMotionPresentation(view, {
      current: (input) => this.owner.current(input),
      rects: () => this.observe(() => readCurrentReorderGeometry(view, { kind: 'complete' })),
      refresh: (input, reason) =>
        reason === 'endpoint' ? this.placement.endpoint(input) : this.refresh(input),
      observe: (action) => this.observe(action),
      schedule: (request) => this.schedule(request),
    });
    this.placement = new ReorderPointerPlacement(view, owner, this.feedback, {
      displayPaint: (input) => this.presentation.displayPaint(input),
      observe: (action) => this.observe(action),
      schedule: (request) => this.schedule(request),
      markerRects: (input) =>
        this.observe(
          () =>
            this.presentation.displayPaint(input)?.geometry ??
            readCurrentReorderGeometry(view, { kind: 'marker', input }),
        ),
      retain: (input) => this.presentation.retain(input),
      before: () => this.presentation.before(),
      propose: (input, origin) => this.presentation.propose(input, origin),
      pending: () => this.presentation.pending(),
      landing: (input, rects, color, viewport, pending) =>
        this.presentation.mark(input, rects, color, viewport, pending),
    });
    this.keyboard = new ReorderKeyboardMeasurement(view, owner, {
      observe: (action) => this.observe(action),
      schedule: (request) => this.schedule(request),
      before: () => this.presentation.before(),
      propose: (input, origin) => this.presentation.propose(input, origin),
      geometry: () =>
        this.observe(() => ({
          rects: readCurrentReorderGeometry(view, { kind: 'complete' }),
          viewport: this.view.scrollDOM.getBoundingClientRect(),
        })),
    });
  }
  /** Unexpected resource observation/publication releases ownership and preserves its original observable cause. */
  private observe<T>(read: () => T, retiring = false): T {
    const epoch = this.epoch;
    try {
      return read();
    } catch (cause) {
      if (epoch + (retiring ? 1 : 0) !== this.epoch) throw cause;
      const failures: unknown[] = [cause];
      try {
        this.owner.failed();
      } catch (cleanup) {
        failures.push(cleanup);
      }
      try {
        if (epoch === this.epoch) this.cancel();
      } catch (cleanup) {
        failures.push(cleanup);
      }
      if (failures.length === 1) throw cause;
      throw new AggregateError(failures, 'Reorder observation cleanup failed');
    }
  }
  /** Scheduling is an external native observation boundary and must release the same finite resources on failure. */
  private schedule<T>(request: {
    read(view: EditorView): T;
    write?(measured: T, view: EditorView): void;
    key?: unknown;
  }): void {
    this.observe(() =>
      this.view.requestMeasure({
        ...request,
        read: (view) => this.inMeasure(() => request.read(view)),
        ...(request.write
          ? {
              write: (measured: T, view: EditorView) =>
                this.inMeasure(() =>
                  requireValue(request.write, 'reorder-measurement')(measured, view),
                ),
            }
          : {}),
      }),
    );
  }
  /** Supported native request callbacks execute inside CM's finite read/write notification phase. */
  private inMeasure<T>(action: () => T): T {
    ++this.measureDepth;
    try {
      return action();
    } finally {
      --this.measureDepth;
    }
  }
  /** Failure retirement can defer dispatch-capable cleanup without inspecting private CM state. */
  get measuring(): boolean {
    return this.measureDepth > 0;
  }
  /** Revoke pending native work immediately while queued retirement retains the exact physical handles. */
  invalidate(): void {
    ++this.epoch;
    ++this.scrollGeneration;
  }
  /** Geometry notification fences the existing native batch before dispatch-capable renewal runs. */
  invalidateGeometry(input: ReorderMeasureInput): void {
    if (this.press?.input === input) this.press.admission = null;
    ++this.scrollGeneration;
    this.keyboard.invalidateGeometry(input);
    this.presentation.invalidateGeometry();
    this.observe(() => this.placement.invalidateGeometry(input));
  }
  /** Entry source coordinates are read only in the native measure phase; preparation happens in the deferred callback. */
  entry(
    start: EditorState,
    coordinates:
      | {
          readonly x: number;
          readonly y: number;
        }
      | undefined,
    settle: (position: number | null) => void,
  ): void {
    const epoch = this.epoch;
    this.schedule({
      key: this,
      read: (view) =>
        this.observe(() => {
          this.feedback.measureAppearance();
          return coordinates ? view.posAtCoords(coordinates) : start.selection.main.head;
        }),
      write: (position) => {
        queueMicrotask(() => {
          if (!this.disposed && epoch === this.epoch) this.observe(() => settle(position));
        });
      },
    });
  }
  /** A new native hold retires the complete prior target/capture lifetime; expected single-epoch cleanup failures retire its session. */
  retarget(
    held?: number,
    press?: {
      readonly x: number;
      readonly y: number;
    },
  ): void {
    const preservePaint =
      held !== undefined &&
      !!press &&
      this.observe(() => this.presentation.reserveGrab(held, press));
    this.observe(() => this.cancel(preservePaint), true);
  }
  /** Native session membership remains authoritative; this returns only an admitted painted identity. */
  visualHit(x: number, y: number): number | null {
    return this.presentation.hit(x, y);
  }
  /** A real primary press reserves native capture; the unchanged chip stays visible until the donor movement threshold. */
  begin(input: ReorderMeasureInput): void {
    if (!this.owner.current(input) || input.pointer === null || !input.coordinates) return;
    this.observe(() => this.capture(input));
  }
  /** Create only this view's noneditable proxy and native pointer capture inside its failure boundary. */
  private capture(input: ReorderMeasureInput): void {
    if (input.pointer === null) return;
    this.pointer = input.pointer;
    const coordinates = input.coordinates;
    if (!coordinates) return;
    const press = {
      input,
      x: coordinates.x,
      y: coordinates.y,
      paint: null as ReorderGrabPaint | null,
      threshold: false,
      release: false,
      admission: null as object | null,
    };
    this.press = press;
    this.view.contentDOM.setPointerCapture(input.pointer);
    const epoch = this.epoch;
    /** Native callbacks cannot authorize a replaced primary press or Session epoch. */
    const owns = (): boolean => !this.disposed && epoch === this.epoch && this.press === press;
    const current = (): boolean => owns() && this.owner.current(input) && owns();
    /** A bounded Scene snapshot refusal first publishes native target paint in WRITE, then samples it in a fresh READ. */
    const measure = (native = false): void =>
      this.schedule({
        key: press,
        read: () =>
          this.observe(() => {
            if (!current()) return null;
            const captured = native ? null : this.presentation.captureGrab(input.held);
            let paint = captured?.kind === 'paint' ? captured.paint : null;
            /** Local ownership is retired before native removal can reenter. */
            const release = (): void => {
              const node = paint?.node;
              paint = null;
              node?.remove();
            };
            try {
              if (!current()) {
                release();
                return null;
              }
              if (captured?.kind === 'native') return { kind: 'native' as const };
              paint ??= captureReorderGrabPaint(this.view, input.held, press);
              if (!current()) {
                release();
                return null;
              }
              const output = { kind: 'paint' as const, paint };
              paint = null;
              return output;
            } catch (primary) {
              try {
                release();
              } catch (cleanup) {
                throw new AggregateError([primary, cleanup], 'reorder-grab-read-retirement-failed');
              }
              throw primary;
            }
          }),
        write: (read) =>
          queueMicrotask(() => {
            if (!owns()) {
              if (read?.kind === 'paint') read.paint?.node.remove();
              return;
            }
            this.observe(() => {
              let paint = read?.kind === 'paint' ? read.paint : null;
              /** Until press takes the snapshot, deferred currentness and cancellation cannot strand it. */
              const release = (): void => {
                const node = paint?.node;
                paint = null;
                node?.remove();
              };
              try {
                if (!current()) {
                  release();
                  return;
                }
                press.paint = paint;
                paint = null;
                this.presentation.cancel();
                if (!current()) return;
                if (read?.kind === 'native') {
                  measure(true);
                  return;
                }
                if (!press.paint) {
                  this.owner.failed();
                  return;
                }
                if (press.threshold) {
                  this.motion(input, press.release);
                  if (!press.release && current()) this.scroll(input);
                }
              } catch (primary) {
                try {
                  release();
                } catch (cleanup) {
                  throw new AggregateError(
                    [primary, cleanup],
                    'reorder-grab-write-retirement-failed',
                  );
                }
                throw primary;
              }
            });
          }),
      });
    measure();
  }
  /** Release/Alt release reserve a crossed threshold even while original press geometry is still pending. */
  pendingDrag(input: ReorderMeasureInput): boolean {
    const press = this.press,
      coordinates = input.coordinates;
    return (
      !!press &&
      press.input === input &&
      (press.threshold ||
        (!!coordinates &&
          Math.abs(coordinates.x - press.x) + Math.abs(coordinates.y - press.y) >=
            reorderDragDistance))
    );
  }
  /** Pointer placement and release bind both measured hidden-output and current coordinate identities. */
  motion(input: ReorderMeasureInput, release: boolean): void {
    const press = this.press;
    if (press?.input === input) {
      const coordinates = input.coordinates;
      if (
        !coordinates ||
        (!press.threshold &&
          Math.abs(coordinates.x - press.x) + Math.abs(coordinates.y - press.y) <
            reorderDragDistance)
      )
        return;
      press.threshold = true;
      press.release ||= release;
      if (press.paint && !press.admission) this.admit(press);
      return;
    }
    const coordinates = input.coordinates;
    if (!this.disposed && input.pointer !== null && coordinates && this.owner.current(input)) {
      const epoch = this.epoch;
      this.observe(() =>
        this.feedback.follow(
          coordinates,
          () => !this.disposed && epoch === this.epoch && this.owner.current(input),
          (action) => this.observe(action),
        ),
      );
    }
    this.placement.motion(input, release);
  }
  /** Bind one exact native epoch and the existing feedback/placement handoff to stateless cold admission. */
  private admit(press: ReorderPointerPress): void {
    const epoch = this.epoch;
    requestReorderPointerAdmission(press, {
      owner: this.owner,
      current: () => !this.disposed && epoch === this.epoch && this.press === press,
      observe: (action) => this.observe(action),
      schedule: (request) => this.schedule(request),
      start: (paint, coordinates, release) => {
        this.press = null;
        this.feedback.begin(paint, coordinates);
        if (
          this.disposed ||
          epoch !== this.epoch ||
          !this.owner.current(press.input) ||
          this.disposed ||
          epoch !== this.epoch
        )
          return;
        this.motion(press.input, release);
        if (!release && this.owner.current(press.input)) this.scroll(press.input);
      },
    });
  }
  /** Native geometry renews keyboard or current pointer feedback without measuring a proposed collision frame. */
  refresh(input: ReorderMeasureInput): void {
    const press = this.press;
    if (press?.input === input) press.admission = null;
    ++this.scrollGeneration;
    this.observe(() => {
      if (this.frame !== null) {
        this.view.dom.ownerDocument.defaultView?.cancelAnimationFrame(this.frame);
        this.frame = null;
      }
    });
    this.observe(() => this.presentation.cancel());
    if (this.disposed || !this.owner.current(input) || this.keyboard.refresh(input)) return;
    if (press && this.press === press && press.input === input && press.threshold && press.paint) {
      this.admit(press);
      return;
    }
    this.placement.refresh(input);
    this.scroll(input);
  }
  /** Explicit wheel keeps browser scroll authority and invalidates queued edge writes without pausing placement. */
  wheel(input: ReorderMeasureInput): void {
    if (this.disposed || input.pointer === null || !input.coordinates || !this.owner.current(input))
      return;
    const coordinates = input.coordinates,
      frame = this.frame;
    ++this.scrollGeneration;
    this.frame = null;
    this.scrollPause = { input, x: coordinates.x, y: coordinates.y };
    if (frame !== null)
      this.observe(() => this.view.dom.ownerDocument.defaultView?.cancelAnimationFrame(frame));
  }
  /** One cancellable RAF resumes after genuine coordinate movement; geometry is read only in the native read callback. */
  scroll(input: ReorderMeasureInput): void {
    const window = this.view.dom.ownerDocument.defaultView;
    if (
      !window ||
      this.frame !== null ||
      input.pointer === null ||
      !input.hidden ||
      !this.owner.current(input)
    )
      return;
    if (this.scrollPause) {
      const coordinates = input.coordinates;
      if (
        this.scrollPause.input === input &&
        coordinates?.x === this.scrollPause.x &&
        coordinates.y === this.scrollPause.y
      )
        return;
      this.scrollPause = null;
    }
    const epoch = this.epoch,
      generation = this.scrollGeneration;
    this.frame = this.observe(() =>
      window.requestAnimationFrame((time) => {
        if (
          this.disposed ||
          epoch !== this.epoch ||
          generation !== this.scrollGeneration ||
          !this.owner.current(input)
        )
          return;
        this.frame = null;
        this.schedule({
          key: this,
          read: () =>
            this.observe(() => {
              if (
                this.disposed ||
                epoch !== this.epoch ||
                generation !== this.scrollGeneration ||
                !this.owner.current(input) ||
                !input.coordinates
              )
                return null;
              const coordinates = input.coordinates,
                box = this.view.scrollDOM.getBoundingClientRect(),
                y = coordinates.y,
                band = Math.min(36, box.height / 4),
                validViewport =
                  Number.isFinite(y) &&
                  Number.isFinite(box.top) &&
                  Number.isFinite(box.bottom) &&
                  Number.isFinite(box.height) &&
                  box.height > 0 &&
                  box.bottom > box.top,
                scale = box.height / this.view.scrollDOM.clientHeight;
              return {
                coordinates,
                direction: !validViewport
                  ? 0
                  : y < box.top + band
                    ? -1
                    : y > box.bottom - band
                      ? 1
                      : 0,
                scale: Number.isFinite(scale) && scale > 0 ? scale : 1,
              };
            }),
          write: (measured) => {
            if (
              this.disposed ||
              epoch !== this.epoch ||
              generation !== this.scrollGeneration ||
              !this.owner.current(input) ||
              input.coordinates !== measured?.coordinates ||
              !measured?.direction
            )
              return;
            if (time - this.lastScroll >= 30) {
              this.lastScroll = time;
              const before = this.view.scrollDOM.scrollTop;
              this.view.scrollDOM.scrollTop += (measured.direction * 24) / measured.scale;
              if (before === this.view.scrollDOM.scrollTop) return;
              this.motion(input, false);
            }
            this.scroll(input);
          },
        });
      }),
    );
  }
  /** Cancellation releases capture/proxy/frame and invalidates every pending read/write/deferred callback. */
  cancel(preservePaint = false): void {
    ++this.epoch;
    this.scrollPause = null;
    const frame = this.frame,
      pointer = this.pointer,
      paint = this.press?.paint;
    this.frame = null;
    this.pointer = null;
    this.press = null;
    const failures: unknown[] = [];
    /** Attempt every captured release even if an earlier external native boundary fails. */
    const release = (action: () => void): void => {
      try {
        action();
      } catch (cause) {
        failures.push(cause);
      }
    };
    release(() => this.keyboard.cancel());
    if (paint) release(() => paint.node.remove());
    if (frame !== null)
      release(() => this.view.dom.ownerDocument.defaultView?.cancelAnimationFrame(frame));
    release(() => this.feedback.close());
    if (!preservePaint) release(() => this.presentation.cancel());
    release(() => this.placement.cancel());
    if (pointer !== null)
      release(() => {
        if (this.view.contentDOM.hasPointerCapture(pointer))
          this.view.contentDOM.releasePointerCapture(pointer);
      });
    if (failures.length === 1) throw failures[0];
    if (failures.length) throw new AggregateError(failures, 'Reorder resource cleanup failed');
  }
  /** Destroy never publishes or reopens resources after native teardown. */
  destroy(): void {
    this.disposed = true;
    const failures: unknown[] = [];
    for (const retire of [
      () => this.keyboard.destroy(),
      () => this.cancel(),
      () => this.placement.destroy(),
      () => this.presentation.destroy(),
    ]) {
      try {
        retire();
      } catch (cause) {
        failures.push(cause);
      }
    }
    if (failures.length === 1) throw failures[0];
    if (failures.length) throw new AggregateError(failures, 'Reorder measurement disposal failed');
  }
}
