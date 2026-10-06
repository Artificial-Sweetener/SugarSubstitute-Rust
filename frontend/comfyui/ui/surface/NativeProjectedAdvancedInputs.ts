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
/** Bind Comfy's advanced footer to the exact projected node and its native disclosure owner. */
import type { ComfyNode } from '../types/graph.js';
import { nativeAdvancedInputControl } from './NativeAdvancedInputControl.js';
import type { NativeAdvancedInputControl } from './NativeAdvancedInputControl.js';

/** Preserve native footer rendering while sharing advanced state with context actions. */
export class NativeProjectedAdvancedInputs {
  readonly #target: HTMLElement;
  readonly #node: ComfyNode;
  readonly #control: NativeAdvancedInputControl | null;

  /** Bind one node rather than routing projected IDs through the active graph. */
  constructor(target: HTMLElement, node: ComfyNode, refresh: () => void) {
    this.#target = target;
    this.#node = node;
    this.#control = nativeAdvancedInputControl(node, refresh);
    target.addEventListener('click', this.#toggle, true);
  }

  /** Keep advanced controls available only on an expanded card with owned advanced fields. */
  reconcile(footer: HTMLElement): void {
    const shown =
      this.#control !== null &&
      this.#node.flags?.collapsed !== true &&
      footer.querySelector('[data-testid="advanced-inputs-button"]') !== null;
    footer.hidden = !shown;
    if (shown) footer.style.removeProperty('display');
    else footer.style.setProperty('display', 'none', 'important');
  }

  /** Return footer interaction to Comfy when its projected card is unmounted. */
  dispose(): void {
    this.#target.removeEventListener('click', this.#toggle, true);
  }

  /** Apply the same exact-node transition used by the context menu. */
  readonly #toggle = (event: Event): void => {
    if (!(event.target instanceof Element) || !this.#control) return;
    const button = event.target.closest<HTMLButtonElement>(
      '[data-testid="advanced-inputs-button"]',
    );
    if (!button || !this.#target.contains(button) || button.disabled) return;
    event.stopImmediatePropagation();
    this.#control.setShown(!this.#control.shown());
  };
}
