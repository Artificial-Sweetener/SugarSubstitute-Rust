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
/** One finite native wheel lifetime arbitrates prepared token intent and delegates canonical edits. */
import { showWeightPreview } from './weight-preview.js';
import { reorderPreview } from './reorder-preview.js';
import { reorderGrabCursor } from './reorder-grab-cursor.js';
import { Transaction } from '@codemirror/state';
import { ViewPlugin, type EditorView, type ViewUpdate } from '@codemirror/view';
import { editorFreshness } from './freshness.js';
import { richRendering } from './presentation.js';
import { weightEditingAllowed, type WeightSubject } from './weight-target.js';
import { weightSession } from './weight-session.js';
import { queueTargetWeight, cancelTargetWeight } from './weight-interactions.js';
import { WheelIntent, type WheelPoint } from './wheel-intent.js';
import { wheelMode } from './wheel-mode.js';
import { measureWheelTarget, wheelHit, type WheelGeometry } from './wheel-geometry.js';
import { showWheelFeedback, wheelFeedback } from './wheel-feedback.js';

/** Finite hover measurements and readiness feedback observe native source/selection without owning either. */
class WheelWeights {
  private readonly view: EditorView;
  private readonly intent = new WheelIntent();
  private target: {
    readonly key: symbol;
    readonly geometry: WheelGeometry;
  } | null = null;
  private activated: WheelWeights['target'] = null;
  private motion: number | null = null;
  private activation: {
    readonly point: WheelPoint;
    readonly freshness: WheelGeometry['freshness'];
  } | null = null;
  private point: WheelPoint | null = null;
  private timer: number | null = null;
  private generation = 0;
  private disposed = false;
  private accented = false;
  /** Application focus loss retires activation even when the range element retains DOM focus. */
  private readonly lostWindowFocus = (): void => this.clear();

  /** Bind every measurement and readiness deadline to one native view. */
  constructor(view: EditorView) {
    this.view = view;
    window.addEventListener('blur', this.lostWindowFocus);
  }

  /** Context replacement cancels authorization; owned steps advance only their exact resulting source target. */
  update(update: ViewUpdate): void {
    const before = update.startState.field(editorFreshness),
      after = update.state.field(editorFreshness);
    if (before.context !== after.context || !this.allowed(this.target?.geometry.kind)) {
      this.clear();
      return;
    }
    if (before.source !== after.source) {
      const session = update.state.field(weightSession);
      if (!this.target || session?.owner !== 'wheel' || session.token !== this.target.key) {
        this.clear();
        return;
      }
      this.target = {
        key: this.target.key,
        geometry: {
          ...this.target.geometry,
          kind: session.kind,
          range: session.range,
          freshness: after,
        },
      };
      if (this.activated?.key === this.target.key) this.activated = this.target;
      this.accented = false;
      if (this.point) showWeightPreview(this.view, session, this.point);
      this.measure();
    } else if (this.point && (update.geometryChanged || update.viewportChanged)) {
      this.geometryChanged();
    }
  }

  /** Native scroll or layout invalidates cached hits immediately; remeasurement preserves the candidate's identity. */
  geometryChanged(): void {
    if (this.target)
      this.target = {
        ...this.target,
        geometry: { ...this.target.geometry, rects: [] },
      };
    this.measure();
  }

  /** Actual pointer movement alone establishes dwell; native clicks activate exactly their measured token. */
  move(event: MouseEvent, activate = false): void {
    if (!this.allowed(this.target?.geometry.kind)) return;
    this.point = { x: event.clientX, y: event.clientY };
    this.motion = performance.now();
    if (activate)
      this.activation = {
        point: this.point,
        freshness: this.view.state.field(editorFreshness),
      };
    this.measure();
  }

  /** Native wheel handlers read only prepared geometry/policy; unauthorized and horizontal input keeps native scrolling. */
  wheel(event: WheelEvent): boolean {
    const target = this.target;
    if (
      !this.allowed(this.target?.geometry.kind) ||
      !target ||
      event.ctrlKey ||
      event.deltaY === 0 ||
      Math.abs(event.deltaX) > Math.abs(event.deltaY) ||
      target.geometry.freshness !== this.view.state.field(editorFreshness) ||
      !wheelHit(target.geometry, { x: event.clientX, y: event.clientY })
    )
      return false;
    if (!this.intent.allow(target.key, this.view.state.facet(wheelMode), performance.now())) {
      this.feedback();
      return false;
    }
    const accepted = queueTargetWeight(this.view, event.deltaY < 0 ? 'increase' : 'decrease', {
      owner: 'wheel',
      token: target.key,
      /** An accepted action retains its captured subject permission when hover moves; mode and lifetime still retire it. */
      current: () => this.allowed(target.geometry.kind),
      freshness: target.geometry.freshness,
      kind: target.geometry.kind,
      range: target.geometry.range,
    });
    if (accepted) {
      this.point = { x: event.clientX, y: event.clientY };
      event.preventDefault();
    }
    return accepted;
  }

  /** Accepted reorder and session-issued pending grabs exclusively own spatial input; ordinary permissions and composition remain independent. */
  private allowed(kind: WeightSubject['kind'] | undefined): boolean {
    return (
      !this.disposed &&
      !this.view.state.field(reorderPreview, false)?.value &&
      !this.view.state.field(reorderGrabCursor, false) &&
      this.view.state.field(weightSession)?.owner !== 'exact' &&
      this.view.state.facet(richRendering) &&
      weightEditingAllowed(this.view.state, kind) &&
      !this.view.composing &&
      !this.view.compositionStarted
    );
  }

  /** Coalesced native measurement performs DOM hit testing outside input and publishes after paint phases. */
  private measure(): void {
    const point = this.point;
    if (!point || !this.allowed(this.target?.geometry.kind)) return;
    const generation = ++this.generation;
    this.view.requestMeasure({
      key: this,
      /** Resolve only the visible prepared token at the captured pointer position. */
      read: () => ({
        geometry: this.allowed(this.target?.geometry.kind)
          ? measureWheelTarget(this.view, point)
          : null,
        activation:
          this.allowed(this.target?.geometry.kind) &&
          this.activation?.freshness === this.view.state.field(editorFreshness)
            ? measureWheelTarget(this.view, this.activation.point)
            : null,
      }),
      /** Policy publication waits until native measurement phases finish. */
      write: ({ geometry, activation }) =>
        queueMicrotask(() => {
          if (this.disposed || generation !== this.generation) return;
          if (!this.allowed(this.target?.geometry.kind)) {
            this.clear();
            return;
          }
          if (!geometry || geometry.freshness !== this.view.state.field(editorFreshness)) {
            this.leave();
            return;
          }
          const motion = this.motion;
          this.motion = null;
          const previous = this.target;
          const same =
            previous?.geometry.kind === geometry.kind &&
            previous?.geometry.range.from === geometry.range.from &&
            previous.geometry.range.to === geometry.range.to;
          const activated = this.activated;
          const returning =
            activated?.geometry.kind === geometry.kind &&
            activated?.geometry.freshness === geometry.freshness &&
            activated.geometry.range.from === geometry.range.from &&
            activated.geometry.range.to === geometry.range.to;
          this.target = {
            key: same ? previous.key : returning ? activated.key : Symbol(),
            geometry,
          };
          if (motion !== null) this.intent.move(this.target.key, point, motion);
          else if (!same) this.intent.leave();
          if (activation) {
            const sameClicked =
              activation.kind === geometry.kind &&
              activation.range.from === geometry.range.from &&
              activation.range.to === geometry.range.to;
            this.activated = sameClicked ? this.target : { key: Symbol(), geometry: activation };
            this.intent.activate(this.activated.key);
          }
          this.activation = null;
          this.feedback();
        }),
    });
  }

  /** A single owned deadline publishes readiness; no periodic polling or semantic work enters timers. */
  private feedback(): void {
    if (this.timer !== null) window.clearTimeout(this.timer);
    this.timer = null;
    const target = this.target,
      allowed = this.allowed(this.target?.geometry.kind);
    const ready =
      allowed &&
      !!target &&
      this.intent.ready(target.key, this.view.state.facet(wheelMode), performance.now());
    if (ready !== this.accented) {
      this.accented = ready;
      this.view.dispatch({
        effects: showWheelFeedback.of(ready && target ? target.geometry : null),
        annotations: Transaction.addToHistory.of(false),
      });
    }
    const remaining =
      allowed && target ? this.intent.remainingDwell(target.key, performance.now()) : null;
    if (
      target &&
      !ready &&
      remaining !== null &&
      this.view.state.facet(wheelMode) === 'hover_dwell'
    )
      this.timer = window.setTimeout(() => {
        this.timer = null;
        if (!this.disposed && this.target === target) {
          if (!this.allowed(this.target?.geometry.kind)) this.clear();
          else this.feedback();
        }
      }, remaining);
  }

  /** Pointer departure ends dwell and pending wheel source commands without retiring keyboard or arrow intent. */
  leave(): void {
    this.generation++;
    this.point = null;
    this.target = null;
    this.motion = null;
    this.activation = null;
    this.intent.leave();
    if (this.timer !== null) window.clearTimeout(this.timer);
    this.timer = null;
    const generation = this.generation;
    queueMicrotask(() => {
      if (this.disposed || generation !== this.generation) return;
      this.feedback();
      cancelTargetWeight(this.view, 'wheel');
    });
  }

  /** Focus, source/context replacement and composition invalidate explicit activation as well as hover. */
  clear(): void {
    this.activated = null;
    this.intent.clear();
    this.leave();
  }

  /** Teardown makes queued measurements inert and releases the one readiness timer. */
  destroy(): void {
    window.removeEventListener('blur', this.lostWindowFocus);
    this.disposed = true;
    this.clear();
  }
}

/** Supported native routing keeps ordinary scrolling under the browser and CodeMirror owners. */
export const wheelWeightInteractions = [
  wheelFeedback,
  ViewPlugin.fromClass(WheelWeights, {
    eventHandlers: {
      /** Consume only a currently authorized vertical token adjustment. */
      wheel(event) {
        return this.wheel(event);
      },
    },
    eventObservers: {
      /** CodeMirror observes editor and ancestor scrolling through its supported native hook. */
      scroll() {
        this.geometryChanged();
      },
      /** Real motion is the only dwell-starting observation. */
      mousemove(event) {
        this.move(event);
      },
      /** Deliberate primary activation preserves native selection/focus handling. */
      mousedown(event) {
        if (event.button === 0) this.move(event, true);
      },
      /** Leaving the editor clears hover and transient neutral continuation. */
      mouseleave() {
        this.leave();
      },
      /** Keyboard authoring/navigation switches to another intent. */
      keydown() {
        this.clear();
      },
      /** Focus transfer clears the token's deliberate activation. */
      blur() {
        this.clear();
      },
      /** Native composition owns unfinished input exclusively. */
      compositionstart() {
        this.clear();
      },
    },
  }),
];
