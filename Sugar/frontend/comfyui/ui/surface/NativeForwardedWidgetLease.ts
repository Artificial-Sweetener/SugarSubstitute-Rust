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
/** Bridge native DOM widget copies whose event closures still target their internal originals. */
import type { ComfyNode } from '../types/graph.js';
import type { NativeNodeWidgetSource } from './NativeNodeCardRenderer.js';

/** Preserve Cube boundary ownership when Comfy reuses an internal DOM element for a promoted widget. */
export class NativeForwardedWidgetLease {
  readonly #release: Array<() => void> = [];

  /** Observe native edits after their own handler, without replacing or cloning controls. */
  constructor(target: HTMLElement, node: ComfyNode, sources: readonly NativeNodeWidgetSource[]) {
    for (const source of sources) {
      const internal = node.widgets?.find((widget) => widget.name === source.targetName);
      const owner = source.node.widgets?.find((widget) => widget.name === source.name);
      if (!internal || !owner) continue;
      let lastValue = internal.value;
      let active = true;
      let pending = false;
      const forward = (): void => {
        if (!active || !pending) return;
        pending = false;
        if (lastValue === internal.value) return;
        lastValue = internal.value;
        const previous = owner.value;
        if (previous === internal.value) return;
        owner.value = internal.value;
        owner.callback?.(owner.value);
        const changed = source.node.onWidgetChanged;
        if (typeof changed === 'function')
          changed.call(source.node, source.name, owner.value, previous, owner);
      };
      const afterNativeEdit = (event: Event): void => {
        const element = internal.element;
        if (
          !(element instanceof HTMLElement) ||
          !(event.target instanceof Node) ||
          !element.contains(event.target)
        )
          return;
        pending = true;
        queueMicrotask(() => queueMicrotask(forward));
      };
      target.addEventListener('input', afterNativeEdit, true);
      target.addEventListener('change', afterNativeEdit, true);
      this.#release.push(() => {
        forward();
        active = false;
        target.removeEventListener('input', afterNativeEdit, true);
        target.removeEventListener('change', afterNativeEdit, true);
      });
    }
  }

  /** Remove forwarding before the native widget returns to graph presentation. */
  dispose(): void {
    for (const release of this.#release.splice(0)) release();
  }
}
