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
/**
 * Open every tooltip beside the pointer. ComfyUI places a tooltip against its target's box: left of
 * a whole field row, above a button. A row that spans a card puts that place on the neighbouring
 * card and its inputs. One rule holds everywhere instead: a tooltip opens beside the pointer,
 * inside the window, like any tooltip on the desktop.
 */

import { HostSurfaceObserver } from '../../core/HostSurfaceObserver.js';

/** Offsets from the pointer, clear of the cursor's own shape. */
export const TOOLTIP_POINTER_OFFSET = { x: 12, y: 20, margin: 8 } as const;
const PLACED = 'data-sc-pointer-tooltip';

export interface TooltipBox {
  readonly width: number;
  readonly height: number;
}

/** Where a tooltip of `box` size opens for a pointer at (x, y) in a viewport, kept fully inside. */
export function pointerTooltipOrigin(
  x: number,
  y: number,
  box: TooltipBox,
  viewport: TooltipBox,
): { left: number; top: number } {
  const { x: dx, y: dy, margin } = TOOLTIP_POINTER_OFFSET;
  const fitsRight = x + dx + box.width + margin <= viewport.width;
  const fitsBelow = y + dy + box.height + margin <= viewport.height;
  const left = fitsRight ? x + dx : x - dx - box.width;
  const top = fitsBelow ? y + dy : y - dy / 2 - box.height;
  return {
    left: Math.max(margin, Math.min(left, viewport.width - box.width - margin)),
    top: Math.max(margin, Math.min(top, viewport.height - box.height - margin)),
  };
}

/** Follow the pointer over the document and place each tooltip as it is added to the body. */
export class PointerTooltipPlacement {
  readonly #observer: HostSurfaceObserver;
  readonly #style: HTMLStyleElement;
  #pointer: { x: number; y: number } | null = null;

  constructor(private readonly document: Document) {
    this.#style = document.createElement('style');
    this.#style.id = 'sc-pointer-tooltip-placement';
    this.#style.textContent = pointerTooltipCss;
    document.head.append(this.#style);
    document.addEventListener('pointermove', this.#onMove, { capture: true, passive: true });
    document.addEventListener('keydown', this.#onKey, { capture: true, passive: true });
    this.#observer = new HostSurfaceObserver(document.body, '.p-tooltip', (changes) => {
      for (const change of changes) if (change.kind === 'mounted') this.place(change.element);
    });
  }

  dispose(): void {
    this.#observer.disconnect();
    this.document.removeEventListener('pointermove', this.#onMove, { capture: true });
    this.document.removeEventListener('keydown', this.#onKey, { capture: true });
    this.#style.remove();
  }

  /** Move one mounted tooltip beside the last pointer position; without one it stays put. */
  place(tooltip: HTMLElement): void {
    const pointer = this.#pointer;
    const view = this.document.defaultView;
    if (!pointer || !view) return;
    const box = tooltip.getBoundingClientRect();
    const { left, top } = pointerTooltipOrigin(pointer.x, pointer.y, box, {
      width: view.innerWidth,
      height: view.innerHeight,
    });
    const fixed = view.getComputedStyle(tooltip).position === 'fixed';
    tooltip.setAttribute(PLACED, '');
    tooltip.style.left = `${String(left + (fixed ? 0 : view.scrollX))}px`;
    tooltip.style.top = `${String(top + (fixed ? 0 : view.scrollY))}px`;
  }

  readonly #onMove = (event: PointerEvent): void => {
    this.#pointer = { x: event.clientX, y: event.clientY };
  };

  /** A tooltip opened from the keyboard has no pointer to stand by and keeps its anchored place. */
  readonly #onKey = (): void => {
    this.#pointer = null;
  };
}

/** A tooltip at the pointer points at nothing, so it has no arrow and no side gap. */
const pointerTooltipCss = `
  .p-tooltip[${PLACED}] { margin:0; padding:0; }
  .p-tooltip[${PLACED}] .p-tooltip-arrow { display:none; }`;
