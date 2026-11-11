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
/** Compose themed advanced disclosure for native graph and stable projected mounts. */
import type { NativeCardBoundary } from './NativeCardAdapter.js';
import { NativeGraphAdvancedDisclosure } from './NativeGraphAdvancedDisclosure.js';
import { NativeProjectedAdvancedDisclosure } from './NativeProjectedAdvancedDisclosure.js';
import { readProjectedAdvancedInputMount } from '../NativeProjectedAdvancedInputs.js';

/** Route presentation to the mounted state owner rather than competing click handlers. */
export class NativeAdvancedDisclosure {
  readonly #buttons = new Map<HTMLButtonElement, Map<string, string | null>>();
  readonly #graph = new NativeGraphAdvancedDisclosure();
  readonly #projected = new NativeProjectedAdvancedDisclosure();

  /** Reconcile each recognized card through its renderer's disclosure lifetime. */
  reconcile(card: NativeCardBoundary): void {
    for (const [button, attributes] of this.#buttons) {
      if (!button.isConnected) {
        restoreButton(button, attributes);
        this.#buttons.delete(button);
      }
    }
    if (card.advanced) {
      const { button, expanded } = card.advanced;
      if (!this.#buttons.has(button))
        this.#buttons.set(
          button,
          new Map(['aria-label', 'aria-expanded'].map((name) => [name, button.getAttribute(name)])),
        );
      button.setAttribute('aria-label', `${expanded ? 'Hide' : 'Show'} advanced inputs`);
      button.setAttribute('aria-expanded', String(expanded));
    }
    const projection = readProjectedAdvancedInputMount(card.root);
    if (projection) this.#projected.reconcile(projection, card);
    else this.#graph.reconcile(card);
  }

  /** Release both presentations while retaining the latest native advanced state. */
  clear(): void {
    this.#graph.clear();
    this.#projected.clear();
    for (const [button, attributes] of this.#buttons) restoreButton(button, attributes);
    this.#buttons.clear();
  }
}

/** Return host accessibility attributes when themed disclosure presentation retires. */
function restoreButton(
  button: HTMLButtonElement,
  attributes: ReadonlyMap<string, string | null>,
): void {
  for (const [name, value] of attributes) {
    if (value === null) button.removeAttribute(name);
    else button.setAttribute(name, value);
  }
}
