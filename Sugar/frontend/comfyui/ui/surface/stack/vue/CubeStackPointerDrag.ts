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
/** Own captured pointer gestures and lifted Vue previews without browser drag payloads. */
import { ref, onBeforeUnmount, type Ref } from 'vue';
import type { CubeStackNavigationRow } from '../CubeStackView.js';
import { CUBE_STACK_ROW_HEIGHT } from '../CubeStackMetrics.js';

interface PointerCandidate {
  readonly id: string;
  readonly pointerId: number;
  readonly startX: number;
  readonly startY: number;
  readonly offsetX: number;
  readonly offsetY: number;
  readonly width: number;
  readonly original: readonly string[];
}

/** Keep presentation gestures independent of canonical graph permission and commit ownership. */
export interface CubeStackPointerOptions {
  root: Ref<HTMLElement | null>;
  rows(): readonly CubeStackNavigationRow[];
  pending(): boolean;
  rowStride(): number;
  preview(order: readonly string[] | null): void;
  commit(id: string, order: readonly string[]): void;
  announce(message: string): void;
}

/** Lift after six pixels, allow every Cube position, and restore canceled previews. */
export function useCubeStackPointerDrag(options: CubeStackPointerOptions) {
  const lifted = ref<{
    row: CubeStackNavigationRow;
    x: number;
    y: number;
    width: number;
    grabX: number;
    grabY: number;
  } | null>(null);
  const order = ref<readonly string[] | null>(null);
  let candidate: PointerCandidate | null = null;
  let suppressClick = false;
  let frame = 0;
  let lastX = 0;
  let lastY = 0;

  /** Release capture and frame work before removing the retained rail. */
  const clear = (cancel = true): void => {
    const root = options.root.value;
    const previous = candidate;
    candidate = null;
    root?.ownerDocument.defaultView?.cancelAnimationFrame(frame);
    frame = 0;
    if (previous && root?.hasPointerCapture?.(previous.pointerId))
      root.releasePointerCapture(previous.pointerId);
    root?.ownerDocument.removeEventListener('keydown', escape, true);
    root?.ownerDocument.removeEventListener('pointermove', move, true);
    root?.ownerDocument.removeEventListener('pointerup', finish, true);
    root?.ownerDocument.defaultView?.removeEventListener('blur', cancelDrag);
    if (cancel && lifted.value) {
      options.announce('Move canceled.');
      options.preview(null);
    }
    lifted.value = null;
    order.value = null;
  };
  const cancelDrag = (): void => {
    if (candidate) {
      clear();
      suppressClick = false;
    }
  };
  const escape = (event: KeyboardEvent): void => {
    if (event.key !== 'Escape') return;
    event.preventDefault();
    event.stopPropagation();
    clear();
  };

  /** Measure settled row slots instead of chasing each neighbor's animated transform. */
  const project = (): void => {
    const root = options.root.value;
    const active = candidate;
    const ghost = lifted.value;
    if (!root || !active || !ghost) return;
    const bounds = root.getBoundingClientRect();
    if (lastX < bounds.left - 24 || lastX > bounds.right + 24) return;
    const list = root.querySelector<HTMLElement>('.sc-cube-stack__rows');
    if (!list) return;
    const current = [...(order.value ?? active.original)];
    const eligible = options.rows().filter((row) => row.reorderable);
    const ids = new Set(eligible.map((row) => row.id));
    const remaining = current.filter((id) => ids.has(id) && id !== active.id);
    const center = ghost.y + CUBE_STACK_ROW_HEIGHT / 2;
    const listTop = list.getBoundingClientRect().top;
    let index = remaining.length;
    for (const [slot, id] of remaining.entries()) {
      const destination = current.indexOf(id) * options.rowStride() + CUBE_STACK_ROW_HEIGHT / 2;
      if (center < listTop + destination) {
        index = slot;
        break;
      }
    }
    remaining.splice(index, 0, active.id);
    let cursor = 0;
    const projected = current.map((id) => (ids.has(id) ? (remaining[cursor++] ?? id) : id));
    if (projected.every((id, slot) => id === current[slot])) return;
    order.value = projected;
    options.preview(projected);
  };

  /** Continue edge scrolling while a captured pointer rests above or below the rail viewport. */
  const edgeScroll = (): void => {
    frame = 0;
    const root = options.root.value;
    const viewport = root?.parentElement;
    if (!root || !viewport || !lifted.value) return;
    const bounds = viewport.getBoundingClientRect();
    const delta = lastY < bounds.top + 32 ? -8 : lastY > bounds.bottom - 32 ? 8 : 0;
    if (delta && lastX >= bounds.left - 24 && lastX <= bounds.right + 24) {
      viewport.scrollTop += delta;
      project();
    }
    frame = root.ownerDocument.defaultView?.requestAnimationFrame(edgeScroll) ?? 0;
  };

  /** Track initial intent without redirecting ordinary clicks away from their buttons. */
  const start = (event: PointerEvent, row: CubeStackNavigationRow): void => {
    const root = options.root.value;
    const target = event.target;
    if (
      !root ||
      candidate ||
      (!row.reorderable && !options.pending()) ||
      event.button !== 0 ||
      (target instanceof Element && target.closest('.sc-cube-stack-row__remove'))
    )
      return;
    const element = event.currentTarget;
    if (!(element instanceof HTMLElement)) return;
    const bounds = element.getBoundingClientRect();
    event.stopPropagation();
    candidate = {
      id: row.id,
      pointerId: event.pointerId,
      startX: event.clientX,
      startY: event.clientY,
      offsetX: event.clientX - bounds.left,
      offsetY: event.clientY - bounds.top,
      width: bounds.width,
      original: options.rows().map((item) => item.id),
    };
    suppressClick = false;
    lastX = event.clientX;
    lastY = event.clientY;
    root.ownerDocument.addEventListener('keydown', escape, true);
    root.ownerDocument.addEventListener('pointermove', move, true);
    root.ownerDocument.addEventListener('pointerup', finish, true);
    root.ownerDocument.defaultView?.addEventListener('blur', cancelDrag);
  };

  /** Resume held intent against the accepted graph order when a preceding commit finishes. */
  const resume = (): void => {
    const active = candidate;
    if (!active || options.pending()) return;
    if (!lifted.value && Math.hypot(lastX - active.startX, lastY - active.startY) < 6) return;
    const row = options.rows().find((item) => item.id === active.id);
    if (!row?.reorderable) {
      clear();
      return;
    }
    if (!lifted.value) {
      candidate = { ...active, original: options.rows().map((item) => item.id) };
      options.root.value?.setPointerCapture(active.pointerId);
    }
    lifted.value = {
      row,
      x: lastX - active.offsetX,
      y: lastY - active.offsetY,
      width: active.width,
      grabX: active.offsetX,
      grabY: active.offsetY,
    };
    suppressClick = true;
    if (!order.value) {
      order.value = candidate?.original ?? active.original;
      options.announce(`Moving ${row.label}. Drop at any Cube position.`);
      frame = options.root.value?.ownerDocument.defaultView?.requestAnimationFrame(edgeScroll) ?? 0;
    }
    project();
  };

  /** Retain pointer motion during a commit without submitting concurrent graph mutations. */
  const move = (event: PointerEvent): void => {
    if (!candidate || event.pointerId !== candidate.pointerId) return;
    event.stopPropagation();
    lastX = event.clientX;
    lastY = event.clientY;
    if (Math.hypot(lastX - candidate.startX, lastY - candidate.startY) >= 6) {
      event.preventDefault();
      suppressClick = true;
    }
    resume();
  };

  /** Commit a changed permutation once; a release outside the rail cancels the gesture. */
  const finish = (event: PointerEvent): void => {
    const active = candidate;
    if (!active || event.pointerId !== active.pointerId) return;
    event.stopPropagation();
    const bounds = options.root.value?.getBoundingClientRect();
    const viewport = options.root.value?.parentElement?.getBoundingClientRect();
    const valid =
      bounds &&
      viewport &&
      event.clientX >= bounds.left - 24 &&
      event.clientX <= bounds.right + 24 &&
      event.clientY >= viewport.top &&
      event.clientY <= viewport.bottom;
    const final = order.value;
    const row = lifted.value?.row;
    if (!valid || !final || !row || final.every((id, index) => id === active.original[index])) {
      clear();
      return;
    }
    clear(false);
    options.commit(active.id, final);
  };
  /** Begin a fresh pointer sequence even when its target is a toolbar rather than a row. */
  const beginPointerInteraction = (): void => {
    if (!candidate) suppressClick = false;
  };
  const click = (event: MouseEvent): void => {
    if (!suppressClick || event.detail === 0) return;
    event.preventDefault();
    event.stopPropagation();
    suppressClick = false;
  };
  onBeforeUnmount(() => clear());
  return {
    lifted,
    order,
    start,
    move,
    resume,
    finish,
    cancel: cancelDrag,
    clear,
    click,
    beginPointerInteraction,
  };
}
