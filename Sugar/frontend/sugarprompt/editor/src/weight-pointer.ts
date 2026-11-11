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
/** One finite pointer lifetime exposes measured token arrows and delegates edits to the native command queue. */
import { showWeightPreview } from './weight-preview.js';
import { reorderPreview } from './reorder-preview.js';
import { reorderGrabCursor } from './reorder-grab-cursor.js';
import { ViewPlugin, type EditorView, type ViewUpdate } from '@codemirror/view';
import { editorFreshness } from './freshness.js';
import { richRendering } from './presentation.js';
import { weightEditingAllowed, steppableLabelSelector } from './weight-target.js';
import { weightSession } from './weight-session.js';
import { cancelTargetWeight, queueTargetWeight } from './weight-interactions.js';
import {
  containsWeightPoint,
  labelTarget,
  measureWeightControls,
  type WeightControlGeometry,
} from './weight-control-geometry.js';
import { WeightControlView, type WeightDirection } from './weight-control-view.js';

/** A held native button owns only timing and pointer intent, never source or caret state. */
interface HeldWeight {
  readonly direction: WeightDirection;
  readonly pointerId: number;
  repeated: boolean;
  aborted: boolean;
}

/** Hover consumes one prepared geometry; finite visible listeners and command work retire with the native view. */
class PointerWeights {
  private readonly view: EditorView;
  private readonly controls: WeightControlView;
  private geometry: WeightControlGeometry | null = null;
  private label: HTMLElement | null = null;
  private held: HeldWeight | null = null;
  private point = { x: 0, y: 0 };
  private repeatTimer: number | null = null;
  private hideTimer: number | null = null;
  private startedAt = 0;
  private generation = 0;
  private disposed = false;
  private observing = false;
  private queued = false;
  /** Bind a native portal and its finite interaction listeners to one CodeMirror view. */
  constructor(view: EditorView) {
    this.view = view;
    this.controls = new WeightControlView(
      (direction, event) => this.press(direction, event),
      (direction) => this.step(direction),
    );
  }
  /** Context/source replacement hides obsolete controls; owned pointer steps refresh their resulting target. */
  update(update: ViewUpdate): void {
    const before = update.startState.field(editorFreshness),
      after = update.state.field(editorFreshness);
    if (before.context !== after.context || !this.allowed()) {
      this.hide();
      return;
    }
    if (before.source !== after.source) {
      const session = update.state.field(weightSession);
      if (!this.geometry || session?.owner !== 'pointer') {
        this.hide();
        return;
      }
      // The owned arrow remains clickable before the next frame; native state already supplies its resulting target.
      this.geometry = {
        ...this.geometry,
        kind: session.kind,
        range: session.range,
        freshness: after,
      };
      this.label = null;
      showWeightPreview(this.view, session, this.point);
      this.measure();
    } else if (
      this.geometry &&
      (update.geometryChanged || update.viewportChanged || update.selectionSet)
    )
      this.measure();
  }
  /** Dormant pointer movement does no geometry work until an actual rendered number is reached. */
  hover(event: MouseEvent): void {
    if (!this.allowed()) return;
    if (
      this.geometry &&
      containsWeightPoint(this.geometry.activation, event.clientX, event.clientY)
    )
      return;
    const label =
      event.target instanceof Element
        ? event.target.closest<HTMLElement>(steppableLabelSelector)
        : null;
    if (!label || this.label === label) return;
    this.hide();
    this.label = label;
    this.point = { x: event.clientX, y: event.clientY };
    this.measure();
  }
  /** Reorder owns token pickup; native composition and host permissions retire source-changing controls. */
  private allowed(): boolean {
    return (
      !this.disposed &&
      !this.view.state.field(reorderPreview, false)?.value &&
      !this.view.state.field(reorderGrabCursor, false) &&
      this.view.state.field(weightSession)?.owner !== 'exact' &&
      this.view.state.facet(richRendering) &&
      weightEditingAllowed(this.view.state, this.geometry?.kind) &&
      !this.view.compositionStarted &&
      !this.view.composing
    );
  }
  /** Resolve the current source-backed label in measurement after a command replaces widget DOM. */
  private currentLabel(): HTMLElement | null {
    if (this.label?.isConnected) return this.label;
    const session = this.view.state.field(weightSession);
    const retained = session?.owner === 'pointer' ? session : this.geometry;
    if (!retained || this.geometry?.freshness !== this.view.state.field(editorFreshness))
      return null;
    const neutral = session?.owner === 'pointer' && session.neutral;
    for (const label of this.view.contentDOM.querySelectorAll<HTMLElement>(
      steppableLabelSelector,
    )) {
      if ((label.dataset.weightNeutral === 'true') !== neutral) continue;
      const target = labelTarget(this.view, label);
      if (
        target?.kind === retained.kind &&
        target.range.from === retained.range.from &&
        target.range.to === retained.range.to
      )
        return label;
    }
    return null;
  }
  /** All geometry reads and portal writes use CodeMirror measurement phases; generation rejects retired results. */
  private measure(): void {
    if (this.queued || this.disposed) return;
    this.queued = true;
    const generation = this.generation;
    this.view.requestMeasure({
      key: this,
      read: () => {
        if (!this.allowed() || generation !== this.generation) return null;
        const label = this.currentLabel();
        return label
          ? {
              label,
              value: measureWeightControls(this.view, label, this.geometry?.centerOffset ?? null),
            }
          : null;
      },
      write: (result) => {
        this.queued = false;
        if (generation !== this.generation || this.disposed) return;
        if (!result?.value) {
          this.hide();
          return;
        }
        this.label = result.label;
        this.geometry = result.value;
        this.controls.show(result.value);
        this.observe(true);
        const held = this.held;
        if (held && !held.aborted) {
          if (!containsWeightPoint(result.value[held.direction], this.point.x, this.point.y))
            this.abortHold();
          else if (held.repeated && this.repeatTimer === null)
            this.armRepeat(performance.now() - this.startedAt >= 2000 ? 80 : 120);
        }
      },
    });
  }
  /** A press starts the reference's deliberate 400ms hold delay; ordinary clicks commit on release. */
  private press(direction: WeightDirection, event: PointerEvent): void {
    if (event.button !== 0 || !this.geometry || !this.allowed()) return;
    event.preventDefault();
    this.clearRepeat();
    this.clearHide();
    this.point = { x: event.clientX, y: event.clientY };
    this.held = {
      direction,
      pointerId: event.pointerId,
      repeated: false,
      aborted: false,
    };
    this.startedAt = performance.now();
    this.controls.setPressed(direction);
    this.armRepeat(400);
  }
  /** Queue one prepared source-current intent; the shared native command owner restores focus and scroll. */
  private step(direction: WeightDirection): boolean {
    const geometry = this.geometry;
    return (
      this.allowed() &&
      geometry !== null &&
      queueTargetWeight(this.view, direction, {
        owner: 'pointer',
        freshness: geometry.freshness,
        kind: geometry.kind,
        range: geometry.range,
      })
    );
  }
  /** Each next hold interval starts after the accepted command's measured geometry publication. */
  private armRepeat(delay: number): void {
    this.repeatTimer = window.setTimeout(() => {
      this.repeatTimer = null;
      const held = this.held,
        geometry = this.geometry;
      if (
        !held ||
        held.aborted ||
        !geometry ||
        !containsWeightPoint(geometry[held.direction], this.point.x, this.point.y) ||
        !this.step(held.direction)
      ) {
        this.abortHold();
        return;
      }
      held.repeated = true;
    }, delay);
  }
  /** Visible pointer motion uses prepared hit rectangles and cancels holds as soon as the pointer leaves its arrow. */
  private readonly moved = (event: Event): void => {
    if (!(event instanceof PointerEvent)) return;
    this.point = { x: event.clientX, y: event.clientY };
    const geometry = this.geometry;
    if (!geometry) return;
    this.controls.setHovered(
      containsWeightPoint(geometry.increase, this.point.x, this.point.y)
        ? 'increase'
        : containsWeightPoint(geometry.decrease, this.point.x, this.point.y)
          ? 'decrease'
          : null,
    );
    if (
      this.held &&
      !containsWeightPoint(geometry[this.held.direction], this.point.x, this.point.y)
    )
      this.abortHold();
    if (containsWeightPoint(geometry.activation, this.point.x, this.point.y)) this.clearHide();
    else if (this.hideTimer === null)
      this.hideTimer = window.setTimeout(() => {
        this.hideTimer = null;
        this.hide();
      }, 140);
  };
  /** A completed repeat never adds a release step; a cancelled press cannot become a late click. */
  private readonly released = (event: Event): void => {
    if (!(event instanceof PointerEvent)) return;
    const held = this.held;
    if (!held || held.pointerId !== event.pointerId) return;
    this.held = null;
    this.clearRepeat();
    this.controls.setPressed(null);
    if (
      !held.aborted &&
      !held.repeated &&
      this.geometry &&
      containsWeightPoint(this.geometry[held.direction], event.clientX, event.clientY)
    )
      this.step(held.direction);
  };
  /** Native cancellation or loss of the active window retires the held gesture without a release step. */
  private readonly cancelled = (event: Event): void => {
    if (
      event.type === 'blur' ||
      (event instanceof PointerEvent && this.held?.pointerId === event.pointerId)
    )
      this.hide();
  };
  /** A new external pointer intent invalidates controls and any queued old-target command. */
  private readonly outside = (event: Event): void => {
    if (event.target instanceof Node && !this.controls.root.contains(event.target)) this.hide();
  };
  /** Authoring/navigation retires pointer continuation; native activation of an arrow remains available. */
  private readonly key = (event: Event): void => {
    if (!(event instanceof KeyboardEvent)) return;
    if (
      !(
        event.target instanceof Node &&
        this.controls.root.contains(event.target) &&
        ['Enter', ' '].includes(event.key)
      )
    )
      this.hide();
  };
  /** Scroll and resize coalesce prepared geometry without changing source or selection. */
  private readonly layout = (): void => {
    if (this.geometry) this.measure();
  };
  /** Visible global listeners exist once and are removed on hide or disposal. */
  private observe(enabled: boolean): void {
    if (this.observing === enabled) return;
    this.observing = enabled;
    const operation = enabled ? 'addEventListener' : 'removeEventListener';
    window[operation]('pointermove', this.moved);
    window[operation]('pointerup', this.released);
    window[operation]('pointercancel', this.cancelled);
    window[operation]('blur', this.cancelled);
    window[operation]('pointerdown', this.outside, true);
    window[operation]('keydown', this.key, true);
    window[operation]('scroll', this.layout, true);
    window[operation]('resize', this.layout);
  }
  /** Clear the one repeat timer without dispatching source changes. */
  private clearRepeat(): void {
    if (this.repeatTimer !== null) window.clearTimeout(this.repeatTimer);
    this.repeatTimer = null;
  }
  /** Clear the reference hide linger when pointer ownership returns. */
  private clearHide(): void {
    if (this.hideTimer !== null) window.clearTimeout(this.hideTimer);
    this.hideTimer = null;
  }
  /** Pointer exit cancels repeating and prevents a later release from mutating source. */
  private abortHold(): void {
    this.clearRepeat();
    if (this.held) this.held.aborted = true;
    this.controls.setPressed(null);
  }
  /** Retire every listener, timer, prepared range and pending pointer command outside native update/paint phases. */
  hide(): void {
    const generation = ++this.generation;
    this.clearRepeat();
    this.clearHide();
    this.observe(false);
    this.held = null;
    this.geometry = null;
    this.queued = false;
    this.label = null;
    this.controls.hide();
    queueMicrotask(() => {
      if (!this.disposed && generation === this.generation)
        cancelTargetWeight(this.view, 'pointer');
    });
  }
  /** Teardown makes measured callbacks inert and releases the owned portal. */
  destroy(): void {
    this.disposed = true;
    this.hide();
    this.controls.destroy();
  }
}

/** Install token pointer controls on the supported native editor lifetime. */
export const pointerWeightInteractions = ViewPlugin.fromClass(PointerWeights, {
  eventObservers: {
    /** Number hover prepares controls; unrelated native motion remains dormant. */
    mousemove(event) {
      this.hover(event);
    },
    /** Preedit owns input and retires pointer commands. */
    compositionstart() {
      this.hide();
    },
  },
});
