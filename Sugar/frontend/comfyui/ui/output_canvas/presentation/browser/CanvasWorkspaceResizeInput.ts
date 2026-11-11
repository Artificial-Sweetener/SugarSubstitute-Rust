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
/** Wire captured divider input to the authoritative workspace allocation and preference owners. */

import type { CanvasFrameClock } from '../../application/CanvasCameraTransition.js';

export interface CanvasWorkspaceResizeOwner {
  /** Read the currently allocated CSS width after authoritative clamping. */
  width(): number;
  /** Preview one requested width without persisting pointer movement. */
  preview(width: number): void;
  /** Persist the completed, already-clamped divider position through the workflow view owner. */
  commit(): void;
  /** Restore accepted allocation after interruption. */
  cancel(): void;
}

/** Keep resize capture, finite frame work, and keyboard handling local to one workspace divider. */
export class CanvasWorkspaceResizeInput {
  readonly #stops: (() => void)[] = [];
  #pointer: { readonly id: number; readonly x: number; readonly width: number } | null = null;
  #pending: number | null = null;
  #frame: (() => void) | null = null;
  #disposed = false;

  /** Bind one explicit separator without window-level drag listeners or graph input forwarding. */
  constructor(
    private readonly element: HTMLElement,
    private readonly frames: CanvasFrameClock,
    private readonly owner: CanvasWorkspaceResizeOwner,
  ) {
    this.#listen('pointerdown', this.#down);
    this.#listen('pointermove', this.#move);
    this.#listen('pointerup', this.#up);
    this.#listen('pointercancel', this.#canceled);
    this.#listen('lostpointercapture', this.#canceled);
    this.#listen('keydown', this.#key);
  }

  /** Cancel captured input before the workspace hides, detaches, or changes its allocation mode. */
  cancel(): void {
    this.#release(false);
  }

  /** Release local handlers and finite work before the separator leaves its document. */
  dispose(): void {
    if (this.#disposed) return;
    this.#disposed = true;
    this.#release(false);
    for (const stop of this.#stops.splice(0)) stop();
  }

  /** Install an exact local DOM listener and its paired teardown. */
  #listen<K extends keyof HTMLElementEventMap>(
    type: K,
    callback: (event: HTMLElementEventMap[K]) => void,
  ): void {
    this.element.addEventListener(type, callback);
    this.#stops.push(() => this.element.removeEventListener(type, callback));
  }

  /** Begin an unmodified primary-button resize from the actual allocated width. */
  readonly #down = (event: PointerEvent): void => {
    if (
      this.#disposed ||
      this.#pointer ||
      event.button !== 0 ||
      event.ctrlKey ||
      event.metaKey ||
      event.altKey ||
      event.shiftKey
    )
      return;
    event.preventDefault();
    event.stopPropagation();
    this.#pointer = { id: event.pointerId, x: event.clientX, width: this.owner.width() };
    this.element.focus({ preventScroll: true });
    this.element.setPointerCapture(event.pointerId);
  };

  /** Retain the latest width and coalesce geometry writes into one animation frame. */
  readonly #move = (event: PointerEvent): void => {
    const pointer = this.#pointer;
    if (!pointer || pointer.id !== event.pointerId) return;
    event.preventDefault();
    event.stopPropagation();
    this.#pending = Math.max(0, pointer.width + pointer.x - event.clientX);
    if (!this.#frame)
      this.#frame = this.frames.request(() => {
        this.#frame = null;
        this.#flush();
      });
  };

  /** Include the final release coordinate and persist one completed resize. */
  readonly #up = (event: PointerEvent): void => {
    if (this.#pointer?.id !== event.pointerId) return;
    this.#move(event);
    this.#release(true);
  };

  /** Restore accepted geometry when the platform cancels this exact captured pointer. */
  readonly #canceled = (event: PointerEvent): void => {
    if (this.#pointer?.id === event.pointerId) this.#release(false);
  };

  /** Apply separator conventions while leaving text, browser, and host shortcuts untouched. */
  readonly #key = (event: KeyboardEvent): void => {
    if (event.ctrlKey || event.metaKey || event.altKey || event.shiftKey) return;
    if (event.key === 'Escape' && this.#pointer) {
      event.preventDefault();
      event.stopPropagation();
      this.#release(false);
      return;
    }
    const width =
      event.key === 'ArrowRight'
        ? Math.max(0, this.owner.width() - 20)
        : event.key === 'ArrowLeft'
          ? this.owner.width() + 20
          : event.key === 'Home'
            ? 1
            : event.key === 'End'
              ? Number.MAX_SAFE_INTEGER
              : null;
    if (width === null) return;
    event.preventDefault();
    event.stopPropagation();
    this.#release(false);
    this.owner.preview(width);
    this.owner.commit();
  };

  /** Apply only the latest pending request through the allocation owner. */
  #flush(): void {
    this.#frame?.();
    this.#frame = null;
    const pending = this.#pending;
    this.#pending = null;
    if (pending !== null && !this.#disposed) this.owner.preview(pending);
  }

  /** Release capture once and avoid persistence for canceled or retired gestures. */
  #release(completed: boolean): void {
    const pointer = this.#pointer;
    this.#pointer = null;
    if (completed) this.#flush();
    else {
      this.#frame?.();
      this.#frame = null;
      this.#pending = null;
    }
    if (pointer && this.element.hasPointerCapture(pointer.id))
      this.element.releasePointerCapture(pointer.id);
    if (completed && pointer) this.owner.commit();
    else if (pointer) this.owner.cancel();
  }
}
