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
/** Adapt real Cube-graph nodes to Comfy's host-owned native card renderer. */

import type { ComfyNode } from '../types/graph.js';
import { createCubeFaceActivationControl } from './CubeFaceActivationControl.js';
import type { CubeFaceCardDecision } from './CubeFaceCardPolicy.js';
import type { NativeNodeCardMount, NativeNodeCardRenderer } from './NativeNodeCardRenderer.js';

/** Own native child-card mounts for one Cube surface view. */
export class NativeNodeCardHost {
  readonly #renderer: NativeNodeCardRenderer;
  readonly #mounts: NativeNodeCardMount[] = [];

  /** Create a host that delegates all node rendering to Comfy. */
  constructor(renderer: NativeNodeCardRenderer) {
    this.#renderer = renderer;
  }

  /** Mount the exact internal graph nodes into dedicated masonry cells. */
  mount(
    container: HTMLElement,
    cards: readonly CubeFaceCardDecision[],
    onActivationChange: (node: ComfyNode, enabled: boolean) => void,
  ): HTMLElement[] {
    this.dispose();
    container.replaceChildren();
    const cells: HTMLElement[] = [];
    for (const card of cards) {
      const { node } = card;
      const cell = container.ownerDocument.createElement('div');
      cell.className = 'sugarcubes-cube-face__node-card';
      cell.dataset.cubeNodeId = String(node.id ?? '');
      cell.dataset.cubeColumnSpan = String(card.columnSpan ?? 1);
      const graphId = node.graph?.id;
      if (graphId !== undefined) {
        cell.dataset.cubeNodeLocator = `${String(graphId)}:${String(node.id ?? '')}`;
      }
      const nativeTarget = container.ownerDocument.createElement('div');
      nativeTarget.className = 'sugarcubes-cube-face__native-card-mount';
      cell.append(nativeTarget);
      container.appendChild(cell);
      this.#mounts.push(this.#renderer.mount(nativeTarget, node));
      if (card.showActivationControl) {
        cell.append(createCubeFaceActivationControl(cell.ownerDocument, card, onActivationChange));
      }
      cells.push(cell);
    }
    return cells;
  }

  /** Refresh render data after an internal node changes structure. */
  refresh(): void {
    for (const mount of this.#mounts) mount.refresh();
  }

  /** Unmount this view's cards while retaining Comfy's shared renderer service. */
  dispose(): void {
    for (const mount of this.#mounts.splice(0)) mount.unmount();
  }
}
