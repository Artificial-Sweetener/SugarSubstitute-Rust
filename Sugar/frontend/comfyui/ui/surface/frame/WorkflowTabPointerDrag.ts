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
/** Own captured workflow-tab gestures, cancellation and one authoritative release command. */
import { workflowTabDragPlacement, workflowTabOffsets } from './WorkflowTabGeometry.js';
import { WorkflowTabDragPreview } from './WorkflowTabDragPreview.js';

interface DragCandidate {
  readonly pointerId: number;
  readonly origin: number;
  readonly x: number;
  readonly y: number;
  readonly scroll: number;
  readonly startLeft: number;
  readonly viewportLeft: number;
  readonly tabs: readonly HTMLElement[];
  readonly widths: readonly number[];
  pointer: number;
  started: boolean;
  target: number;
}

/** Coordinate presentation with host reorder while leaving selection to official Fluent focus. */
export class WorkflowTabPointerDrag {
  readonly #preview: WorkflowTabDragPreview;
  #tabs: readonly HTMLElement[] = [];
  #candidate: DragCandidate | null = null;
  #suppressClick = false;
  #scrollFrame: number | null = null;
  #awaitingOrder = false;
  readonly #document: Document;

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
    viewport.addEventListener('pointerdown', this.#down);
    viewport.addEventListener('scroll', this.#scrolled);
    viewport.addEventListener('click', this.#click, true);
    viewport.addEventListener('lostpointercapture', this.#lostCapture);
    viewport.addEventListener('contextmenu', this.#cancel, true);
    this.#document.addEventListener('pointermove', this.#move);
    this.#document.addEventListener('pointerup', this.#up);
    this.#document.addEventListener('pointercancel', this.#cancel);
    this.#document.addEventListener('keydown', this.#key);
    this.#document.defaultView?.addEventListener('blur', this.#cancel);
  }

  /** Keep canonical updates from overwriting an active preview's orb and outline geometry. */
  get active(): boolean {
    return this.#preview.active;
  }

  /** Cancel invalidated gestures and capture painted positions before keyed DOM projection. */
  beforeUpdate(next: readonly string[]): ReadonlyMap<HTMLElement, number> | null {
    const changed =
      next.length !== this.#tabs.length || next.some((id, index) => id !== this.#tabs[index]?.id);
    if (changed && this.#candidate) this.#release();
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
    this.#suppressClick = false;
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
    const viewportLeft = this.viewport.getBoundingClientRect().left;
    this.#candidate = {
      pointerId: event.pointerId,
      origin,
      x: event.clientX,
      y: event.clientY,
      scroll: this.viewport.scrollLeft,
      startLeft: tab.getBoundingClientRect().left - viewportLeft + this.viewport.scrollLeft,
      viewportLeft,
      pointer: event.clientX,
      tabs: this.#tabs,
      widths: [...this.tabWidths()],
      started: false,
      target: origin,
    };
    event.preventDefault();
    tab.focus({ preventScroll: true });
    // A stationary release must keep the compatibility click on the official tab.
    tab.setPointerCapture(event.pointerId);
  };

  /** Resolve pointer motion from immutable slots rather than moving neighbor rectangles. */
  readonly #move = (event: PointerEvent): void => {
    const candidate = this.#candidate;
    if (!candidate || event.pointerId !== candidate.pointerId) return;
    if (!(event.buttons & 1)) {
      this.#cancel();
      return;
    }
    candidate.pointer = event.clientX;
    if (
      !candidate.started &&
      Math.hypot(event.clientX - candidate.x, event.clientY - candidate.y) < 6
    )
      return;
    candidate.started = true;
    event.preventDefault();
    this.#project();
    this.#edgeScroll();
  };

  /** Account for scroll movement without changing the original press-relative grab position. */
  #placement(candidate: DragCandidate) {
    const placement = workflowTabDragPlacement(
      candidate.widths,
      candidate.origin,
      candidate.pointer -
        candidate.x +
        candidate.viewportLeft -
        this.viewport.getBoundingClientRect().left +
        this.viewport.scrollLeft -
        candidate.scroll +
        candidate.startLeft -
        (workflowTabOffsets(candidate.widths)[candidate.origin] ?? 0),
      candidate.target,
    );
    candidate.target = placement.target;
    return placement;
  }

  /** Project the current captured pointer once per input or autoscroll frame. */
  #project(): void {
    const candidate = this.#candidate;
    if (candidate?.started)
      this.#preview.show(candidate.tabs, candidate.origin, this.#placement(candidate));
  }

  /** Preserve the grab position when wheel input or host scrolling changes the viewport. */
  readonly #scrolled = (): void => this.#project();

  /** Reproject a captured tab after endpoint controls move the viewport beneath its pointer. */
  viewportChanged(): void {
    this.#project();
  }

  /** Continue reaching offscreen slots while a held pointer rests at the viewport edge. */
  #edgeScroll(): void {
    if (this.#scrollFrame !== null) return;
    const window = this.#document.defaultView;
    if (!window) return;
    this.#scrollFrame = window.requestAnimationFrame(() => {
      this.#scrollFrame = null;
      const candidate = this.#candidate;
      if (!candidate?.started) return;
      const rect = this.viewport.getBoundingClientRect();
      const step =
        candidate.pointer < rect.left + 32 ? -8 : candidate.pointer > rect.right - 32 ? 8 : 0;
      const before = this.viewport.scrollLeft;
      this.viewport.scrollLeft = Math.max(
        0,
        Math.min(this.viewport.scrollWidth - this.viewport.clientWidth, before + step),
      );
      if (this.viewport.scrollLeft !== before) {
        this.#project();
        this.#edgeScroll();
      }
    });
  }

  /** Commit exactly once on release, preserving the preview until Comfy projects accepted order. */
  readonly #up = (event: PointerEvent): void => {
    const candidate = this.#candidate;
    if (!candidate || event.pointerId !== candidate.pointerId) return;
    candidate.pointer = event.clientX;
    const placement = this.#placement(candidate);
    this.#project();
    this.#release();
    if (!candidate.started) return;
    this.#suppressClick = true;
    if (placement.target === candidate.origin) {
      this.#settle();
      return;
    }
    this.#awaitingOrder = true;
    try {
      this.reorder(candidate.origin, placement.target);
    } catch (error: unknown) {
      this.#settle();
      this.report(error);
    }
  };

  /** Release capture after clearing the candidate so lost-capture cannot cancel a valid commit. */
  #release(): void {
    const candidate = this.#candidate;
    this.#candidate = null;
    if (this.#scrollFrame !== null)
      this.#document.defaultView?.cancelAnimationFrame(this.#scrollFrame);
    this.#scrollFrame = null;
    const tab = candidate?.tabs[candidate.origin];
    if (candidate && tab?.hasPointerCapture(candidate.pointerId))
      tab.releasePointerCapture(candidate.pointerId);
  }

  /** Return to accepted order on cancellation and host action failure. */
  #settle(): void {
    this.#awaitingOrder = false;
    if (this.#preview.active) this.#preview.settle(this.#tabs, this.#preview.capture(this.#tabs));
  }

  /** Cancel implicit capture, keyboard escape, menus and focus loss with the same settle path. */
  readonly #cancel = (event?: Event): void => {
    if (
      event &&
      'pointerId' in event &&
      this.#candidate &&
      event.pointerId !== this.#candidate.pointerId
    )
      return;
    if (this.#candidate?.started) this.#suppressClick = true;
    this.#release();
    this.#settle();
  };

  /** Ignore the expected capture-loss event after release has already accepted a command. */
  readonly #lostCapture = (event: PointerEvent): void => {
    if (this.#candidate) this.#cancel(event);
  };

  /** Preserve keyboard activation while swallowing only the drag's compatibility click. */
  readonly #click = (event: MouseEvent): void => {
    if (this.#suppressClick && event.detail !== 0) {
      event.preventDefault();
      event.stopImmediatePropagation();
      this.#suppressClick = false;
    }
  };

  /** Consume Escape only while this strip owns a pointer candidate. */
  readonly #key = (event: KeyboardEvent): void => {
    if (event.key === 'Escape' && this.#candidate) {
      event.preventDefault();
      event.stopPropagation();
      this.#cancel();
    }
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
    this.viewport.removeEventListener('click', this.#click, true);
    this.viewport.removeEventListener('lostpointercapture', this.#lostCapture);
    this.viewport.removeEventListener('contextmenu', this.#cancel, true);
    this.#document.removeEventListener('pointermove', this.#move);
    this.#document.removeEventListener('pointerup', this.#up);
    this.#document.removeEventListener('pointercancel', this.#cancel);
    this.#document.removeEventListener('keydown', this.#key);
    this.#document.defaultView?.removeEventListener('blur', this.#cancel);
  }
}
