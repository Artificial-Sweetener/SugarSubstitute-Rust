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
/** Reorder workflow tabs by dragging, on the gesture every rearranged list shares. */
import { ReorderGesture, type LiftedCard } from '../../interaction/reorder/ReorderGesture.js';
import { extentPlacement, slotOffsets } from '../../interaction/reorder/ReorderSlots.js';
import { WorkflowTabDragPreview } from './WorkflowTabDragPreview.js';

/** The tab being pressed or dragged, with the strip as it was when the press began. */
interface PressedTab {
  readonly pointerId: number;
  readonly origin: number;
  readonly tab: HTMLElement;
  readonly tabs: readonly HTMLElement[];
  readonly widths: readonly number[];
  /** The slot the tab hangs over, kept so a tab resting on a boundary does not flicker. */
  target: number;
}

/**
 * Coordinate presentation with host reorder while leaving selection to official Fluent focus.
 *
 * The shared gesture owns the press, the lift, cancellation and the release. This owner adds what
 * a tab strip needs: the tab keeps the pointer from the press so a stationary release still clicks
 * it, the dragged tab stays on the strip and follows it as it scrolls, and a drop waits for ComfyUI
 * to project the accepted order before another drag begins.
 */
export class WorkflowTabPointerDrag {
  readonly #preview: WorkflowTabDragPreview;
  readonly #gesture: ReorderGesture;
  readonly #document: Document;
  #tabs: readonly HTMLElement[] = [];
  #pressed: PressedTab | null = null;
  #awaitingOrder = false;
  /** The tab list changed under a gesture: it ends without the usual return to accepted order. */
  #superseded = false;

  /** Bind one delegated pointer lifetime to the scrolling workflow strip. */
  constructor(
    private readonly viewport: HTMLElement,
    list: HTMLElement,
    private readonly reorder: (from: number, to: number) => void,
    private readonly report: (error: unknown) => void,
    restored: () => void,
    private readonly tabWidths: () => readonly number[],
  ) {
    this.#document = viewport.ownerDocument;
    this.#preview = new WorkflowTabDragPreview(list, viewport, restored, tabWidths);
    this.#gesture = new ReorderGesture({
      document: this.#document,
      threshold: 6,
      // A stationary release must keep the compatibility click on the official tab.
      capture: 'press',
      propagatePress: true,
      captureElement: () => this.#pressed?.tab ?? null,
      admission: () => (this.#awaitingOrder ? 'refuse' : 'ready'),
      keys: () => this.#tabs.map((tab) => tab.id),
      project: (card) => this.#project(card),
      scroll: { viewport: () => this.viewport, axis: 'x', edge: 32, step: 8, lateral: Infinity },
      lifted: () => undefined,
      moved: () => undefined,
      preview: () => undefined,
      commit: (key, order) => this.#commit(order.indexOf(key)),
      cancelled: () => this.#ended(),
    });
    viewport.addEventListener('pointerdown', this.#down);
    viewport.addEventListener('scroll', this.#scrolled);
    viewport.addEventListener('click', this.#gesture.click, true);
    viewport.addEventListener('lostpointercapture', this.#lostCapture);
    viewport.addEventListener('contextmenu', this.#cancel, true);
    this.#document.addEventListener('pointermove', this.#released, true);
    this.#document.addEventListener('pointercancel', this.#pointerCancelled);
  }

  /** Keep canonical updates from overwriting an active preview's orb and outline geometry. */
  get active(): boolean {
    return this.#preview.active;
  }

  /** Cancel invalidated gestures and capture painted positions before keyed DOM projection. */
  beforeUpdate(next: readonly string[]): ReadonlyMap<HTMLElement, number> | null {
    const changed =
      next.length !== this.#tabs.length || next.some((id, index) => id !== this.#tabs[index]?.id);
    if (changed && this.#gesture.pressed) {
      // The update itself settles the strip, from the positions captured below.
      this.#superseded = true;
      this.#gesture.clear();
      this.#superseded = false;
    }
    return this.#preview.active && changed ? this.#preview.capture(this.#tabs) : null;
  }

  /** Settle accepted order while preserving the identity of every official tab control. */
  afterUpdate(
    tabs: readonly HTMLElement[],
    positions: ReadonlyMap<HTMLElement, number> | null,
  ): void {
    this.#tabs = tabs;
    if (positions) {
      this.#awaitingOrder = false;
      this.#preview.settle(tabs, positions);
    }
  }

  /** Arm only primary tab-body presses, excluding existing close and menu controls. */
  readonly #down = (event: PointerEvent): void => {
    this.#gesture.beginInteraction();
    if (event.button !== 0 || !event.isPrimary || this.#awaitingOrder) return;
    const path = event.composedPath();
    if (
      path.some(
        (target) => target instanceof Element && target.classList.contains('sc-workflow-close'),
      )
    )
      return;
    const origin = this.#tabs.findIndex((tab) => path.includes(tab));
    const tab = this.#tabs[origin];
    if (!tab) return;
    this.#cancel();
    this.#pressed = {
      pointerId: event.pointerId,
      origin,
      tab,
      tabs: this.#tabs,
      widths: [...this.tabWidths()],
      target: origin,
    };
    event.preventDefault();
    tab.focus({ preventScroll: true });
    if (!this.#gesture.press(event, tab.id, tab)) this.#pressed = null;
  };

  /**
   * Place the dragged tab where the lifted card hangs and name the order a drop there gives.
   *
   * The card's left edge is read against the strip as it is now, so the tab keeps its grab
   * position while the strip scrolls beneath the pointer.
   */
  #project(card: LiftedCard): readonly string[] | null {
    const pressed = this.#pressed;
    if (!pressed) return null;
    const placement = extentPlacement(
      pressed.widths,
      pressed.origin,
      card.x -
        this.viewport.getBoundingClientRect().left +
        this.viewport.scrollLeft -
        (slotOffsets(pressed.widths)[pressed.origin] ?? 0),
      pressed.target,
    );
    pressed.target = placement.target;
    this.#preview.show(pressed.tabs, pressed.origin, placement);
    const order = pressed.tabs.map((tab) => tab.id);
    const [moved] = order.splice(pressed.origin, 1);
    if (moved !== undefined) order.splice(placement.target, 0, moved);
    return order;
  }

  /** Preserve the grab position when wheel input or host scrolling changes the viewport. */
  readonly #scrolled = (): void => this.#gesture.reproject();

  /** Reproject a captured tab after endpoint controls move the viewport beneath its pointer. */
  viewportChanged(): void {
    this.#gesture.reproject();
  }

  /** Commit exactly once on release, preserving the preview until Comfy projects accepted order. */
  #commit(target: number): void {
    const pressed = this.#pressed;
    this.#pressed = null;
    if (!pressed) return;
    this.#awaitingOrder = true;
    try {
      this.reorder(pressed.origin, target);
    } catch (error: unknown) {
      this.#settle();
      this.report(error);
    }
  }

  /** A press or drag ended without a drop: return to the accepted order. */
  #ended(): void {
    this.#pressed = null;
    if (!this.#superseded) this.#settle();
  }

  /** Return to accepted order on cancellation and host action failure. */
  #settle(): void {
    this.#awaitingOrder = false;
    if (this.#preview.active) this.#preview.settle(this.#tabs, this.#preview.capture(this.#tabs));
  }

  /** Cancel implicit capture, menus and resizes with the same settle path. */
  readonly #cancel = (): void => {
    if (this.#gesture.pressed) this.#gesture.clear();
    else this.#settle();
  };

  /** A move that arrives without the primary button means its release was never delivered. */
  readonly #released = (event: PointerEvent): void => {
    if (this.#pressed && event.pointerId === this.#pressed.pointerId && !(event.buttons & 1))
      this.#cancel();
  };

  readonly #pointerCancelled = (event: PointerEvent): void => {
    if (this.#pressed && event.pointerId === this.#pressed.pointerId) this.#cancel();
  };

  /** Ignore the expected capture-loss event after release has already accepted a command. */
  readonly #lostCapture = (event: PointerEvent): void => {
    if (this.#pressed && event.pointerId === this.#pressed.pointerId) this.#cancel();
  };

  /** Retire a gesture before its slot allocation changes. */
  cancelForResize(): void {
    this.#cancel();
  }

  /** Remove every delegated listener, capture, autoscroll and animation on frame retirement. */
  dispose(): void {
    this.#cancel();
    this.#preview.dispose();
    this.viewport.removeEventListener('pointerdown', this.#down);
    this.viewport.removeEventListener('scroll', this.#scrolled);
    this.viewport.removeEventListener('click', this.#gesture.click, true);
    this.viewport.removeEventListener('lostpointercapture', this.#lostCapture);
    this.viewport.removeEventListener('contextmenu', this.#cancel, true);
    this.#document.removeEventListener('pointermove', this.#released, true);
    this.#document.removeEventListener('pointercancel', this.#pointerCancelled);
  }
}
