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
/** Observe only an identified surface's current composed ancestry. */

/** Keep ancestor attributes and direct child changes within one native observation owner. */
export class HostAncestryObserver {
  readonly #observer: MutationObserver | null;
  readonly #attributes: string[];

  /** Deliver batched changes without traversing or observing the document subtree. */
  constructor(
    window: Window & typeof globalThis,
    changed: () => void,
    attributes: readonly string[] = [],
  ) {
    const Observer = window.MutationObserver;
    this.#observer = Observer ? new Observer(changed) : null;
    this.#attributes = [
      ...new Set(['class', 'style', 'hidden', 'open', 'slot', 'name', ...attributes]),
    ];
  }

  /** Replace exact targets after a verified ancestry change. */
  observe(surface: Element, ancestors: readonly Element[], roots: readonly ShadowRoot[]): void {
    this.#observer?.disconnect();
    for (const node of ancestors) {
      this.#observer?.observe(node, {
        attributes: true,
        attributeFilter: this.#attributes,
        childList: node !== surface,
      });
    }
    for (const root of roots) this.#observer?.observe(root, { childList: true });
  }

  /** Retire native observation with the surface. */
  disconnect(): void {
    this.#observer?.disconnect();
  }
}
