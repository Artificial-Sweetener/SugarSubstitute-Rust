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
/** Publish exact live node ownership independently of serialized IDs and presentation location. */
import type { ComfyNode } from '../types/graph.js';

/** Identify a renderer mount within its owning Cube instance. */
export interface NativeNodeCardBoundary {
  readonly node: ComfyNode;
  readonly cubeId: string | null;
}

const boundaries = new WeakMap<HTMLElement, NativeNodeCardBoundary>();

/** Bind the renderer's actual node until its card mount is released. */
export function registerNativeNodeCard(
  root: HTMLElement,
  node: ComfyNode,
  cubeId: string | null = null,
): () => void {
  const boundary = { node, cubeId };
  boundaries.set(root, boundary);
  return () => {
    if (boundaries.get(root) === boundary) boundaries.delete(root);
  };
}

/** Resolve the nearest positively registered card through graph and masonry relocation. */
export function readNativeNodeCard(element: HTMLElement): NativeNodeCardBoundary | null {
  for (let root: HTMLElement | null = element; root; root = root.parentElement) {
    const boundary = boundaries.get(root);
    if (boundary) return boundary;
  }
  return null;
}
