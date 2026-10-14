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
/** Route deliberate, local image gestures without taking host or browser shortcuts. */

import type { CanvasAnchor } from '../../domain/CanvasCamera.js';

export interface BrowserCanvasGestures {
  /** Test the actual displayed footprint in viewport coordinates. */
  contains(point: CanvasAnchor): boolean;
  /** Stop any obsolete camera animation before continuous input. */
  interrupt(): void;
  /** Translate the displayed camera without inertia. */
  pan(delta: CanvasAnchor): void;
  /** Zoom through the camera's anchored snap policy. */
  zoom(factor: number): void;
  /** Normalize and apply a local wheel gesture. */
  wheel(delta: number, units: 0 | 1 | 2, anchor: CanvasAnchor): void;
  /** Combine centroid motion with a two-pointer scale change. */
  pinch(factor: number, anchor: CanvasAnchor, delta: CanvasAnchor): void;
  /** Toggle the displayed Fit/native camera at a pointer. */
  doubleClick(anchor: CanvasAnchor): void;
  /** Fit the displayed image. */
  fit(): void;
  /** Admit fitted mouse dragging through a qualified platform transfer owner. */
  outbound?(event: MouseEvent): boolean;
}

interface TouchTap {
  readonly point: CanvasAnchor;
  readonly time: number;
}

/** Own pointer capture and keyboard/wheel listeners for one browser image viewport. */
export class BrowserCanvasInput {
  readonly #points = new Map<number, CanvasAnchor>();
  readonly #stops: (() => void)[] = [];
  #engaged = false;
  #tapStart: TouchTap | null = null;
  #lastTap: TouchTap | null = null;
  #disposed = false;

  /** Attach local listeners; overlay controls retain their own input and scrolling. */
  constructor(
    private readonly element: HTMLElement,
    private readonly gestures: BrowserCanvasGestures,
  ) {
    element.tabIndex = 0;
    element.setAttribute('role', 'region');
    element.setAttribute('aria-label', 'Output image viewer');
    element.dataset.captureWheel = 'true';
    this.#listen('pointerdown', (event) => this.#down(event));
    this.#listen('pointermove', (event) => this.#move(event));
    this.#listen('pointerup', (event) => this.#finish(event, true));
    this.#listen('pointercancel', (event) => this.#finish(event, false));
    this.#listen('lostpointercapture', (event) => this.#finish(event, false));
    this.#listen('dblclick', (event) => {
      const point = this.#point(event);
      if (!this.gestures.contains(point)) return;
      event.preventDefault();
      event.stopPropagation();
      this.gestures.doubleClick(point);
    });
    this.#listen(
      'wheel',
      (event) => {
        event.stopPropagation();
        if (
          !this.#engaged ||
          event.ctrlKey ||
          event.metaKey ||
          event.altKey ||
          event.shiftKey ||
          !this.gestures.contains(this.#point(event)) ||
          ![0, 1, 2].includes(event.deltaMode)
        )
          return;
        event.preventDefault();
        this.gestures.wheel(event.deltaY, wheelUnits(event.deltaMode), this.#point(event));
      },
      { passive: false },
    );
    this.#listen('keydown', (event) => this.#key(event));
    this.#listen('focus', () => {
      this.#engaged = true;
    });
    this.#listen('blur', () => this.cancel());
    this.#listen('dragstart', (event) => {
      if (!this.gestures.outbound?.(event)) event.preventDefault();
    });
  }

  /** Release captured pointers and engagement when the attachment hides or input is interrupted. */
  cancel(): void {
    this.#engaged = false;
    this.#tapStart = null;
    this.#lastTap = null;
    const pointers = [...this.#points.keys()];
    this.#points.clear();
    for (const id of pointers)
      if (this.element.hasPointerCapture(id)) this.element.releasePointerCapture(id);
    this.element.classList.remove('sc-output-dragging');
    this.gestures.interrupt();
  }

  /** Remove exactly this viewport's listeners and pointer ownership. */
  dispose(): void {
    if (this.#disposed) return;
    this.#disposed = true;
    this.cancel();
    for (const stop of this.#stops.splice(0)) stop();
  }

  /** Admit only deliberate primary presses on displayed image pixels. */
  #down(event: PointerEvent): void {
    const point = this.#point(event);
    if (
      this.#disposed ||
      event.button !== 0 ||
      this.#points.size >= 2 ||
      event.ctrlKey ||
      event.metaKey ||
      event.altKey ||
      event.shiftKey ||
      !this.gestures.contains(point)
    )
      return;
    this.#engaged = true;
    this.element.focus({ preventScroll: true });
    this.gestures.interrupt();
    if (event.pointerType !== 'touch' && this.gestures.outbound?.(event)) return;
    event.preventDefault();
    event.stopPropagation();
    this.#points.set(event.pointerId, point);
    this.element.setPointerCapture(event.pointerId);
    this.element.classList.add('sc-output-dragging');
    this.#tapStart =
      event.pointerType === 'touch' && this.#points.size === 1
        ? { point, time: event.timeStamp }
        : null;
    if (this.#points.size === 2) this.#lastTap = null;
  }

  /** Rebase every event from the existing pointers, so adding/removing a finger cannot jump. */
  #move(event: PointerEvent): void {
    const previous = this.#points.get(event.pointerId);
    if (!previous) return;
    event.preventDefault();
    event.stopPropagation();
    const before = [...this.#points.values()];
    const point = this.#point(event);
    this.#points.set(event.pointerId, point);
    if (this.#tapStart && distance(this.#tapStart.point, point) > 8) this.#tapStart = null;
    const after = [...this.#points.values()];
    const first = before[0];
    const next = after[0];
    if (!first || !next) return;
    const second = before[1];
    const nextSecond = after[1];
    if (!second || !nextSecond) {
      this.gestures.pan({ x: point.x - previous.x, y: point.y - previous.y });
      return;
    }
    const anchor = midpoint(first, second);
    const center = midpoint(next, nextSecond);
    const span = distance(first, second);
    this.gestures.pinch(span > 0 ? distance(next, nextSecond) / span : 1, anchor, {
      x: center.x - anchor.x,
      y: center.y - anchor.y,
    });
  }

  /** End one pointer while leaving a surviving finger anchored at its last position. */
  #finish(event: PointerEvent, completed: boolean): void {
    if (!this.#points.delete(event.pointerId)) return;
    event.stopPropagation();
    if (this.element.hasPointerCapture(event.pointerId))
      this.element.releasePointerCapture(event.pointerId);
    if (!this.#points.size) this.element.classList.remove('sc-output-dragging');
    const start = this.#tapStart;
    this.#tapStart = null;
    if (!completed || !start || event.pointerType !== 'touch' || event.timeStamp - start.time > 300)
      return;
    const point = this.#point(event);
    if (distance(start.point, point) > 8) return;
    const last = this.#lastTap;
    if (last && event.timeStamp - last.time < 350 && distance(last.point, point) < 24) {
      this.#lastTap = null;
      this.gestures.doubleClick(point);
    } else this.#lastTap = { point, time: event.timeStamp };
  }

  /** Consume only viewer keys from its own focus, leaving text and modified shortcuts intact. */
  #key(event: KeyboardEvent): void {
    if (event.target !== this.element || event.ctrlKey || event.metaKey || event.altKey) return;
    const delta: Record<string, CanvasAnchor> = {
      ArrowLeft: { x: 40, y: 0 },
      ArrowRight: { x: -40, y: 0 },
      ArrowUp: { x: 0, y: 40 },
      ArrowDown: { x: 0, y: -40 },
    };
    const pan = delta[event.key];
    if (event.key === 'Escape') this.cancel();
    else if (pan && !event.shiftKey) this.gestures.pan(pan);
    else if (event.key === '+' || event.key === '=') this.gestures.zoom(1.25);
    else if (event.key === '-') this.gestures.zoom(0.8);
    else if (event.key === '0' && !event.shiftKey) this.gestures.fit();
    else return;
    event.preventDefault();
    event.stopPropagation();
  }

  /** Convert input to viewport CSS coordinates without depending on graph transforms. */
  #point(event: MouseEvent): CanvasAnchor {
    const bounds = this.element.getBoundingClientRect();
    return { x: event.clientX - bounds.left, y: event.clientY - bounds.top };
  }

  /** Keep event types precise and unregister the exact installed callback. */
  #listen<K extends keyof HTMLElementEventMap>(
    type: K,
    listener: (event: HTMLElementEventMap[K]) => void,
    options?: AddEventListenerOptions,
  ): void {
    this.element.addEventListener(type, listener, options);
    this.#stops.push(() => this.element.removeEventListener(type, listener, options));
  }
}

/** Translate the already validated browser wheel unit discriminator. */
function wheelUnits(value: number): 0 | 1 | 2 {
  return value === 1 ? 1 : value === 2 ? 2 : 0;
}

/** Measure pointer travel and span in the same CSS coordinate space. */
function distance(first: CanvasAnchor, second: CanvasAnchor): number {
  return Math.hypot(second.x - first.x, second.y - first.y);
}

/** Find the centroid used for a two-pointer anchored gesture. */
function midpoint(first: CanvasAnchor, second: CanvasAnchor): CanvasAnchor {
  return { x: (first.x + second.x) / 2, y: (first.y + second.y) / 2 };
}
