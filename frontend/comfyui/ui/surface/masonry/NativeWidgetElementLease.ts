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
/** Restore singleton custom widget elements after temporary native card mounts. */
import type { ComfyNode } from '../../types/graph.js';

/** Remember widget DOM ownership without copying controls or changing their callbacks. */
export class NativeWidgetElementLease {
  readonly #entries: { element: HTMLElement; parent: Node; next: ChildNode | null }[] = [];

  /** Capture graph-rendered singleton elements before Comfy adopts them into masonry. */
  constructor(nodes: readonly ComfyNode[]) {
    for (const node of nodes) {
      for (const widget of node.widgets ?? []) {
        const element = widget.element;
        if (element instanceof HTMLElement && element.parentNode) {
          this.#entries.push({ element, parent: element.parentNode, next: element.nextSibling });
        }
      }
    }
  }

  /** Return surviving widgets to their original graph mounts after native unmount completes. */
  restore(): void {
    for (const { element, parent, next } of this.#entries) {
      if (!parent.isConnected) continue;
      parent.insertBefore(element, next?.parentNode === parent ? next : null);
    }
    this.#entries.length = 0;
  }
}
