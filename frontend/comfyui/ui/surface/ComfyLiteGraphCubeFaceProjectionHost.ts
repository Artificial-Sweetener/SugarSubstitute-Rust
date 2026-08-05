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
/** Project embedded Nodes 1.0 node-local DOM affordances onto Cube-face cards. */

import type { ComfyNode } from '../types/graph.js';
import type { CubeCanvasCard, CubeCanvasLayout } from './CubeCanvasLayout.js';

/** Identify the host projection contract exposed on embedded LiteGraph nodes. */
export const CUBE_FACE_PROJECTION_SYMBOL = Symbol.for('sugarcubes.cube-face-projection.v1');

/** Describe one projected DOM rectangle in viewport pixels. */
export interface CubeFaceProjectionBounds {
  left: number;
  top: number;
  width: number;
  height: number;
}

/** Expose a Cube card's DOM container and graph-to-viewport projection. */
export interface CubeFaceProjection {
  container: HTMLElement;
  projectRect(rect: readonly [number, number, number, number]): CubeFaceProjectionBounds;
}

/** Provide the LiteGraph canvas transform required for DOM projection. */
export interface LiteGraphCubeFaceProjectionCanvas {
  canvas: HTMLCanvasElement;
  ds?: {
    scale?: number;
    offset?: ArrayLike<number>;
  };
}

/** Pair one active Cube with the canvas layout used by its projections. */
export interface LiteGraphCubeFaceProjectionItem {
  layout: CubeCanvasLayout;
}

interface MountedProjection {
  node: ComfyNode;
  container: HTMLDivElement;
  originalDescriptor: PropertyDescriptor | undefined;
  card: CubeCanvasCard;
}

/** Own the reversible cross-extension projection seam for visible Cube cards. */
export class ComfyLiteGraphCubeFaceProjectionHost {
  readonly #document: Document;
  readonly #canvas: LiteGraphCubeFaceProjectionCanvas;
  readonly #logger: Pick<Console, 'warn'> | null;
  readonly #mounted = new Map<ComfyNode, MountedProjection>();
  #root: HTMLDivElement | null = null;
  #suspended = false;

  /** Bind projection geometry to one active LiteGraph canvas. */
  constructor(options: {
    document: Document;
    canvas: LiteGraphCubeFaceProjectionCanvas;
    logger?: Pick<Console, 'warn'>;
  }) {
    this.#document = options.document;
    this.#canvas = options.canvas;
    this.#logger = options.logger ?? null;
    this.#canvas.canvas.addEventListener('pointerdown', this.#suspend);
    this.#document.addEventListener('pointerup', this.#resume, true);
    this.#document.addEventListener('pointercancel', this.#resume, true);
  }

  /** Reconcile projection owners with the currently visible Cube-face cards. */
  sync(items: readonly LiteGraphCubeFaceProjectionItem[]): void {
    const desired = new Map<ComfyNode, CubeCanvasCard>();
    for (const item of items) {
      for (const card of item.layout.cards) desired.set(card.node, card);
    }
    for (const [node, mounted] of [...this.#mounted]) {
      if (!desired.has(node)) this.#unmount(mounted);
    }
    for (const [node, card] of desired) {
      const mounted = this.#mounted.get(node) ?? this.#mount(node, card);
      if (!mounted) continue;
      mounted.card = card;
      this.#position(mounted);
    }
    this.#removeEmptyRoot();
  }

  /** Restore every node property and release the viewport host. */
  dispose(): void {
    this.#canvas.canvas.removeEventListener('pointerdown', this.#suspend);
    this.#document.removeEventListener('pointerup', this.#resume, true);
    this.#document.removeEventListener('pointercancel', this.#resume, true);
    for (const mounted of [...this.#mounted.values()]) this.#unmount(mounted);
    this.#root?.remove();
    this.#root = null;
  }

  readonly #suspend = (): void => {
    this.#suspended = true;
    if (this.#root) this.#root.hidden = true;
  };

  readonly #resume = (): void => {
    if (!this.#suspended) return;
    this.#suspended = false;
    if (this.#root) this.#root.hidden = false;
  };

  /** Install one non-serializing projection value on an embedded native node. */
  #mount(node: ComfyNode, card: CubeCanvasCard): MountedProjection | null {
    const container = this.#document.createElement('div');
    container.dataset.sugarcubesCubeFaceProjection = '';
    Object.assign(container.style, {
      position: 'fixed',
      transformOrigin: '0 0',
      boxSizing: 'border-box',
      overflow: 'hidden',
      pointerEvents: 'none',
    });
    const mounted: MountedProjection = {
      node,
      container,
      originalDescriptor: Object.getOwnPropertyDescriptor(node, CUBE_FACE_PROJECTION_SYMBOL),
      card,
    };
    const projection: CubeFaceProjection = {
      container,
      projectRect: (rect) => this.#project(mounted.card, rect),
    };
    try {
      Object.defineProperty(node, CUBE_FACE_PROJECTION_SYMBOL, {
        configurable: true,
        enumerable: false,
        value: projection,
      });
    } catch (error: unknown) {
      this.#logger?.warn('SugarCubes could not expose Cube-face projection geometry.', {
        nodeId: node.id,
        error,
      });
      return null;
    }
    this.#ensureRoot().append(container);
    this.#mounted.set(node, mounted);
    return mounted;
  }

  /** Position one clipped DOM surface over its complete native face card. */
  #position(mounted: MountedProjection): void {
    const { x, y, width, height } = mounted.card.rect;
    const scale = this.#scale();
    const bounds = this.#canvas.canvas.getBoundingClientRect();
    const offset = this.#canvas.ds?.offset;
    mounted.container.style.left = `${String(bounds.left + (x + vector(offset, 0)) * scale)}px`;
    mounted.container.style.top = `${String(bounds.top + (y + vector(offset, 1)) * scale)}px`;
    mounted.container.style.width = `${String(Math.max(1, width))}px`;
    mounted.container.style.height = `${String(Math.max(1, height))}px`;
    mounted.container.style.transform = `scale(${String(scale)})`;
  }

  /** Convert a native node-local body rectangle into viewport geometry. */
  #project(
    card: CubeCanvasCard,
    rect: readonly [number, number, number, number],
  ): CubeFaceProjectionBounds {
    const [left, top, width, height] = rect;
    const titleHeight = Math.max(1, card.rect.height - card.bodyHeight);
    const scale = this.#scale();
    const bounds = this.#canvas.canvas.getBoundingClientRect();
    const offset = this.#canvas.ds?.offset;
    return {
      left: bounds.left + (card.rect.x + left + vector(offset, 0)) * scale,
      top: bounds.top + (card.rect.y + titleHeight + top + vector(offset, 1)) * scale,
      width: Math.max(0, width * scale),
      height: Math.max(0, height * scale),
    };
  }

  /** Restore the exact prior symbol descriptor and remove its clipped surface. */
  #unmount(mounted: MountedProjection): void {
    if (mounted.originalDescriptor) {
      Object.defineProperty(mounted.node, CUBE_FACE_PROJECTION_SYMBOL, mounted.originalDescriptor);
    } else {
      Reflect.deleteProperty(mounted.node, CUBE_FACE_PROJECTION_SYMBOL);
    }
    mounted.container.remove();
    this.#mounted.delete(mounted.node);
  }

  /** Create one pointer-transparent viewport layer for all projected cards. */
  #ensureRoot(): HTMLDivElement {
    if (this.#root) return this.#root;
    const root = this.#document.createElement('div');
    root.dataset.sugarcubesCubeFaceProjections = '';
    Object.assign(root.style, {
      position: 'fixed',
      inset: '0',
      overflow: 'hidden',
      pointerEvents: 'none',
      zIndex: '2',
    });
    root.hidden = this.#suspended;
    this.#document.body.append(root);
    this.#root = root;
    return root;
  }

  /** Remove the projection layer as soon as no visible card needs it. */
  #removeEmptyRoot(): void {
    if (this.#mounted.size > 0) return;
    this.#root?.remove();
    this.#root = null;
  }

  /** Read the active finite graph scale with a safe neutral fallback. */
  #scale(): number {
    const scale = Number(this.#canvas.ds?.scale);
    return Number.isFinite(scale) && scale > 0 ? scale : 1;
  }
}

/** Read one finite graph transform component with a neutral fallback. */
function vector(value: ArrayLike<number> | undefined, index: number): number {
  const component = Number(value?.[index]);
  return Number.isFinite(component) ? component : 0;
}
