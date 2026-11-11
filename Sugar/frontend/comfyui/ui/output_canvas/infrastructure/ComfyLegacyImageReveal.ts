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
/** Place an accessible hover action above Nodes 1 image widgets without changing their drawing or pointer behavior. */
import { isRecord, type UnknownRecord } from '../../types/common.js';
import type { CanvasRevealTarget } from '../domain/CanvasReveal.js';
import { createImageRevealButton } from './ComfyImageRevealDom.js';

type Draw = (this: UnknownRecord, ...args: unknown[]) => unknown;

/** Borrow foreground callbacks only from cached output sources; use Comfy's existing render clock. */
export class ComfyLegacyImageReveal {
  readonly #bindings = new Map<UnknownRecord, { original: unknown; wrapped: Draw }>();
  #current: { node: UnknownRecord; target: CanvasRevealTarget; button: HTMLButtonElement } | null =
    null;

  /** Bind the current host canvas, exact file resolution, and renderer-independent reveal command. */
  constructor(
    private readonly document: Document,
    private readonly canvas: () => UnknownRecord | null,
    private readonly target: (
      path: string,
      url: string,
      batchIndex: number | null,
    ) => CanvasRevealTarget | null,
    private readonly reveal: (target: CanvasRevealTarget) => void,
  ) {}

  /** Retain live node callback identities and release retired or inactive renderer bindings. */
  reconcile(sources: readonly { path: string; node: UnknownRecord }[]): void {
    const active = new Set(sources.map((s) => s.node));
    for (const [node, binding] of this.#bindings)
      if (!active.has(node)) {
        if (node.onDrawForeground === binding.wrapped) node.onDrawForeground = binding.original;
        this.#bindings.delete(node);
        if (this.#current?.node === node) {
          this.#current.button.remove();
          this.#current = null;
        }
      }
    for (const { node, path } of sources) {
      if (this.#bindings.has(node)) continue;
      const original: unknown = node.onDrawForeground;
      const owner = this;
      const wrapped: Draw = function (...args) {
        const result =
          typeof original === 'function' ? Reflect.apply(original, this, args) : undefined;
        owner.#draw(node, path);
        return result;
      };
      node.onDrawForeground = wrapped;
      this.#bindings.set(node, { original, wrapped });
    }
  }

  /** Restore host callbacks and remove this adapter's single floating action. */
  dispose(): void {
    this.reconcile([]);
    this.#current?.button.remove();
    this.#current = null;
  }

  /** Position at most one button over the pointer's actual completed native image. */
  #draw(node: UnknownRecord, path: string): void {
    const canvas = this.canvas();
    const element = canvas?.canvas;
    const ds = isRecord(canvas?.ds) ? canvas.ds : null;
    if (!(element instanceof HTMLCanvasElement) || !ds || typeof ds.scale !== 'number') {
      this.#clear(node);
      return;
    }
    const mouse = vector(canvas?.graph_mouse),
      pos = vector(node.pos),
      offset = vector(ds.offset),
      size = vector(node.size);
    const widgets = Array.isArray(node.widgets) ? node.widgets : [];
    const widget = widgets.find((w: unknown) => isRecord(w) && w.name === '$$canvas-image-preview');
    if (!mouse || !pos || !offset || !size || !isRecord(widget)) {
      this.#clear(node);
      return;
    }
    const images = Array.isArray(node.imgs) ? node.imgs : [];
    const chosen =
      typeof node.imageIndex === 'number' ? node.imageIndex : images.length === 1 ? 0 : null;
    const rects: unknown[] =
      chosen !== null
        ? [[0, widget.y, size[0], widget.computedHeight]]
        : Array.isArray(node.imageRects)
          ? node.imageRects
          : [];
    const local: readonly [number, number] = [mouse[0] - pos[0], mouse[1] - pos[1]];
    const index = rects.findIndex(
      (rect: unknown) =>
        Array.isArray(rect) &&
        rect.length >= 4 &&
        rect.every((v: unknown) => typeof v === 'number' && Number.isFinite(v)) &&
        local[0] >= Number(rect[0]) &&
        local[1] >= Number(rect[1]) &&
        local[0] <= Number(rect[0]) + Number(rect[2]) &&
        local[1] <= Number(rect[1]) + Number(rect[3]),
    );
    const image: unknown = index >= 0 ? images[chosen ?? index] : null;
    const target =
      isRecord(image) && typeof image.src === 'string'
        ? this.target(path, image.src, chosen ?? index)
        : null;
    const rect = rects[index];
    if (!target || !Array.isArray(rect)) {
      this.#clear(node);
      return;
    }
    if (this.#current?.node !== node || this.#current.target.itemId !== target.itemId) {
      this.#current?.button.remove();
      const button = createImageRevealButton(this.document, () => {
        this.#current?.button.remove();
        this.#current = null;
        this.reveal(target);
      });
      button.classList.add('sc-image-reveal-legacy');
      this.document.body.append(button);
      this.#current = { node, target, button };
    }
    const bounds = element.getBoundingClientRect();
    const x = bounds.left + (pos[0] + Number(rect[0]) + offset[0]) * ds.scale + 8;
    const y = bounds.top + (pos[1] + Number(rect[1]) + offset[1]) * ds.scale + 8;
    this.#current.button.style.left = `${x}px`;
    this.#current.button.style.top = `${y}px`;
  }

  /** Keep pointer transfer to the floating action interactive until focus or hover leaves it. */
  #clear(node: UnknownRecord): void {
    if (this.#current?.node !== node || this.#current.button.matches(':hover,:focus-within'))
      return;
    this.#current.button.remove();
    this.#current = null;
  }
}

/** Narrow host vectors without propagating dynamic graph values into policy. */
function vector(value: unknown): readonly [number, number] | null {
  if (!isRecord(value) && !Array.isArray(value)) return null;
  const x: unknown = Reflect.get(value, '0'),
    y: unknown = Reflect.get(value, '1');
  return typeof x === 'number' && Number.isFinite(x) && typeof y === 'number' && Number.isFinite(y)
    ? [x, y]
    : null;
}
