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
/** Position Nodes 1.0 native output slots across from Cube preview titles. */

import type { CubeNode } from '../cube/node/ComfyCubeNodeFactory.js';
import type { CubeCanvasPort } from './CubeCanvasLayout.js';

interface LiteGraphSlot {
  pos?: unknown;
}

interface SlotPresentation {
  hadPosition: boolean;
  position: unknown;
}

export interface ComfyLiteGraphCubeBoundaryHostOptions {
  slotHeight?: number;
}

/** Own reversible presentation changes to real LiteGraph output slots. */
export class ComfyLiteGraphCubeBoundaryHost {
  readonly #presentations = new Map<CubeNode, Map<LiteGraphSlot, SlotPresentation>>();
  readonly #inputClipRight = new Map<CubeNode, number>();
  readonly #slotHeight: number;

  /** Bind Comfy's native slot geometry without owning its visual styling. */
  constructor(options: ComfyLiteGraphCubeBoundaryHostOptions = {}) {
    this.#slotHeight = readPositiveNumber(
      options.slotHeight,
      readPositiveNumber(globalThis.LiteGraph?.NODE_SLOT_HEIGHT, 20),
    );
  }

  /** Synchronize native output hit targets with current preview-title geometry. */
  sync(node: CubeNode, outputs: readonly CubeCanvasPort[], inputClipRight?: number): void {
    const presentations = this.#presentations.get(node) ?? new Map();
    this.#presentations.set(node, presentations);
    this.#inputClipRight.set(node, Math.max(0, inputClipRight ?? Number(node.size[0]) / 2));
    const current = new Set<LiteGraphSlot>();
    for (const output of outputs) {
      const slot = asLiteGraphSlot(node.outputs[output.index]);
      if (!slot) continue;
      current.add(slot);
      if (!presentations.has(slot)) {
        presentations.set(slot, {
          hadPosition: Object.hasOwn(slot, 'pos'),
          position: slot.pos,
        });
      }
      slot.pos = [
        resolveNativeOutputSlotX(Number(node.size[0]), this.#slotHeight),
        output.y - Number(node.pos[1]),
      ];
    }
    for (const slot of presentations.keys()) {
      if (!current.has(slot)) {
        restoreSlot(slot, presentations.get(slot));
        presentations.delete(slot);
      }
    }
  }

  /** Draw native slots through geometry that excludes duplicate output labels. */
  drawNativeSlotsWithoutOutputLabels(
    node: CubeNode,
    context: CanvasRenderingContext2D,
    drawSlots: () => void,
  ): void {
    const width = Number(node.size[0]);
    const height = Number(node.size[1]);
    const inputClipRight = Math.min(width, this.#inputClipRight.get(node) ?? width / 2);
    context.save();
    context.beginPath();
    context.rect(-16, -64, inputClipRight + 16, height + 128);
    for (const value of node.outputs) {
      const slot = asLiteGraphSlot(value);
      const position = asPosition(slot?.pos);
      if (!position) continue;
      context.moveTo(position[0] + 8, position[1]);
      context.arc(position[0], position[1], 8, 0, Math.PI * 2);
    }
    context.clip();
    try {
      drawSlots();
    } finally {
      context.restore();
    }
  }

  /** Restore every output slot owned by one released Cube node. */
  release(node: CubeNode): void {
    const presentations = this.#presentations.get(node);
    if (!presentations) return;
    for (const [slot, presentation] of presentations) restoreSlot(slot, presentation);
    this.#presentations.delete(node);
    this.#inputClipRight.delete(node);
  }

  /** Restore all mounted Cube output slots. */
  dispose(): void {
    for (const node of [...this.#presentations.keys()]) this.release(node);
  }
}

/** Match LiteGraph's native output-slot inset for ordinary Nodes 1.0 cards. */
function resolveNativeOutputSlotX(nodeWidth: number, slotHeight: number): number {
  return nodeWidth + 1 - slotHeight / 2;
}

/** Narrow one dynamic host slot to the presentation surface LiteGraph reads. */
function asLiteGraphSlot(value: unknown): LiteGraphSlot | null {
  return typeof value === 'object' && value !== null && !Array.isArray(value)
    ? (value as LiteGraphSlot)
    : null;
}

/** Restore the exact optional properties present before Cube presentation. */
function restoreSlot(slot: LiteGraphSlot, presentation: SlotPresentation | undefined): void {
  if (!presentation) return;
  restoreOptional(slot, 'pos', presentation.hadPosition, presentation.position);
}

/** Narrow an optional host slot position to finite local canvas coordinates. */
function asPosition(value: unknown): readonly [number, number] | null {
  if (!Array.isArray(value) && !ArrayBuffer.isView(value)) return null;
  const x = Number(Reflect.get(value, '0'));
  const y = Number(Reflect.get(value, '1'));
  return Number.isFinite(x) && Number.isFinite(y) ? [x, y] : null;
}

/** Read a finite positive host geometry value without propagating invalid state. */
function readPositiveNumber(value: unknown, fallback: number): number {
  const number = Number(value);
  return Number.isFinite(number) && number > 0 ? number : fallback;
}

/** Restore one optionally owned host property without inventing undefined state. */
function restoreOptional<K extends keyof LiteGraphSlot>(
  slot: LiteGraphSlot,
  key: K,
  existed: boolean,
  value: LiteGraphSlot[K],
): void {
  if (existed) {
    slot[key] = value;
  } else {
    Reflect.deleteProperty(slot, key);
  }
}
