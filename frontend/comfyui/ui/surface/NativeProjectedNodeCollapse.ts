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
/** Route projected collapse gestures to the exact native node instead of the active graph's ID lookup. */
import type { ComfyNode } from '../types/graph.js';
import { CardDisclosureMotion } from './motion/CardDisclosureMotion.js';
import { isNativeWidgetPresented } from './NativeWidgetPresentation.js';
import { cubeFaceRevealableWidgets } from './CubeFaceWidgetPolicy.js';
import { settleNativeDisclosure } from './NativeDisclosureSettlement.js';

/** Adapt a native collapse button while leaving flags, history, and graph geometry with Comfy. */
export class NativeProjectedNodeCollapse {
  readonly #target: HTMLElement;
  readonly #node: ComfyNode;
  readonly #refresh: () => void;
  readonly #motion: CardDisclosureMotion;

  /** Bind one projected card and its native renderer refresh. */
  constructor(
    target: HTMLElement,
    node: ComfyNode,
    refresh: () => void,
    motion: CardDisclosureMotion,
  ) {
    this.#target = target;
    this.#node = node;
    this.#refresh = refresh;
    this.#motion = motion;
    target.addEventListener('click', this.#collapse, true);
  }

  /** Reflect the native node's collapse capability in the existing control. */
  reconcile(
    root: HTMLElement,
    hasContent = cubeFaceRevealableWidgets(this.#node).some(isNativeWidgetPresented),
  ): void {
    const button = root.querySelector<HTMLButtonElement>('[data-testid="node-collapse-button"]');
    if (button) {
      button.hidden = !hasContent;
      if (button.hidden) button.style.setProperty('display', 'none', 'important');
      else button.style.removeProperty('display');
      button.disabled =
        button.hidden ||
        typeof this.#node.collapse !== 'function' ||
        this.#node.collapsible === false;
    }
  }

  /** Release the host-facing listener before the Vue mount is unmounted. */
  dispose(): void {
    this.#target.removeEventListener('click', this.#collapse, true);
  }

  /** Invoke the authoritative node transition without consulting ambiguous numeric IDs. */
  readonly #collapse = (event: Event): void => {
    if (!(event.target instanceof Element)) return;
    const button = event.target.closest<HTMLButtonElement>('[data-testid="node-collapse-button"]');
    if (
      !button ||
      !this.#target.contains(button) ||
      button.disabled ||
      typeof this.#node.collapse !== 'function'
    )
      return;
    event.stopImmediatePropagation();
    this.#motion.toggle(
      this.#node.flags?.collapsed === true,
      (collapsed) => {
        if (collapsed !== (this.#node.flags?.collapsed === true)) {
          this.#node.graph?.beforeChange?.();
          this.#node.collapse?.();
          this.#node.graph?.afterChange?.();
        }
        this.#refresh();
      },
      { settle: () => settleNativeDisclosure(this.#target) },
    );
  };
}
