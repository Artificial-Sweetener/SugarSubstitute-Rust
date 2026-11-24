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
/** Let a prompt editor offer its own search painting to whoever finds its element. */

/** One match in a prompt's exact source text, in UTF-16 units. */
export interface PromptSearchRange {
  readonly start: number;
  readonly length: number;
}

/** A prompt that paints search matches in its own text renderer. */
export interface PromptSearchSurface {
  /** The prompt's element, for bringing it into view. */
  readonly element: HTMLElement;
  /**
   * Show every match of one query, with `active` the index of the current one. `query` names
   * the query so a new one starts a new search; it is never shown.
   */
  paint(query: string, ranges: readonly PromptSearchRange[], active: number | null): void;
  /** Select the match at `index`, scroll the prompt to it and optionally give it focus. */
  reveal(index: number, focus: boolean): void;
  /** Remove every match. */
  clear(): void;
}

// The editor runs from its own bundle, so the element carries the surface under a key both
// bundles resolve to the same symbol.
const key = Symbol.for('sugarcubes.prompt-search-surface');
type PromptElement = HTMLElement & { [key]?: PromptSearchSurface };

/** Offer a prompt editor's search painting on its root element. */
export function publishPromptSearch(surface: PromptSearchSurface): void {
  (surface.element as PromptElement)[key] = surface;
}

/** The search painting of the prompt editor inside `row`, when it has one. */
export function promptSearchIn(row: HTMLElement): PromptSearchSurface | null {
  const element = row.querySelector<PromptElement>('[data-sc-prompt-editor]');
  return element?.[key] ?? null;
}
