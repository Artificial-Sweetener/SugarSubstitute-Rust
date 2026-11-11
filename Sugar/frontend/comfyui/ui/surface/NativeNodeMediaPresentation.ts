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
/** Replace native media in an exact projected card without modifying its graph-owned image state. */
import type { ComfyNode } from '../types/graph.js';

const attribute = 'data-sc-native-media';
const owners = new WeakMap<HTMLElement, NativeNodeMediaReplacement>();

/** Read the nearest projected media replacement without discovering nodes from DOM state. */
export function isNativeNodeMediaPresented(element: HTMLElement): boolean {
  return element.closest(`[${attribute}="replaced"]`) === null;
}

/** Own a reversible media presentation lease for one identified native card. */
export class NativeNodeMediaReplacement {
  readonly #previous: string | null;
  /** Suppress Comfy's source-file media only inside the owning card's replacement surface. */
  constructor(private readonly root: HTMLElement) {
    if (owners.has(root)) throw new Error('Native card media has competing presentation owners.');
    owners.set(root, this);
    this.#previous = root.getAttribute(attribute);
    root.setAttribute(attribute, 'replaced');
    const document = root.ownerDocument;
    if (!document.getElementById('sc-native-media-presentation')) {
      const style = document.createElement('style');
      style.id = 'sc-native-media-presentation';
      style.textContent = `[${attribute}="replaced"] .lg-node-content{display:none!important;}`;
      document.head.append(style);
    }
  }
  /** Restore only the exact media presentation lease still owned by this lifetime. */
  dispose(): void {
    if (owners.get(this.root) !== this) return;
    owners.delete(this.root);
    if (this.#previous === null) this.root.removeAttribute(attribute);
    else this.root.setAttribute(attribute, this.#previous);
  }
}

/** Hide legacy media only during measurement and drawing, restoring exact graph property ownership. */
export function withNativeNodeMediaPresentation<Result>(
  element: HTMLElement,
  node: ComfyNode,
  draw: () => Result,
): Result {
  if (isNativeNodeMediaPresented(element)) return draw();
  const keys = ['imgs', 'animatedImages'] as const;
  const remembered = keys.map((key) => ({
    key,
    value: Reflect.get(node, key),
    owned: Object.hasOwn(node, key),
  }));
  for (const key of keys) Reflect.set(node, key, []);
  try {
    return draw();
  } finally {
    for (const { key, value, owned } of remembered) {
      Reflect.set(node, key, value);
      if (!owned) Reflect.deleteProperty(node, key);
    }
  }
}
