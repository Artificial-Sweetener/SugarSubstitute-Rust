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
/** Give jsdom the highlight registry and range geometry a browser has, and read what is shown. */
import {
  SEARCH_CURRENT_HIGHLIGHT,
  SEARCH_MATCH_HIGHLIGHT,
} from '../../../frontend/sugarprompt/services/src/search/highlights.js';

const registry = new Map<string, Set<Range>>();

/** Install the registry on the window; call once per test file. */
export function installSearchHighlights(): void {
  Object.defineProperty(window, 'Highlight', { value: Set, configurable: true });
  Object.defineProperty(window.CSS, 'highlights', { value: registry, configurable: true });
  // jsdom lays nothing out; a range reports the empty box its elements do.
  if (typeof Range.prototype.getBoundingClientRect !== 'function')
    Object.defineProperty(Range.prototype, 'getBoundingClientRect', {
      configurable: true,
      value: () => ({ left: 0, top: 0, right: 0, bottom: 0, width: 0, height: 0, x: 0, y: 0 }),
    });
}

/** Forget every range between tests. */
export function resetSearchHighlights(): void {
  registry.clear();
}

const describe = (name: string): string[] =>
  [...(registry.get(name) ?? [])].map(
    (range) => `${range.toString()}@${range.startContainer.parentElement?.tagName ?? ''}`,
  );

/** Every match that is not the current one, as `text@TAG` of the element holding the text. */
export const shownMatches = (): string[] => describe(SEARCH_MATCH_HIGHLIGHT);
/** The current match outside prompt editors, as `text@TAG`. */
export const shownCurrent = (): string[] => describe(SEARCH_CURRENT_HIGHLIGHT);
/** The matches marked in text boxes' copies under `root`, as `text:current` or `text:match`. */
export const boxRuns = (root: ParentNode): string[] =>
  [...root.querySelectorAll<HTMLElement>('.sc-search-highlight [data-sc-search]')].map(
    (run) => `${run.textContent ?? ''}:${run.dataset.scSearch ?? ''}`,
  );
/** Whether either highlight is still registered. */
export const anySearchHighlight = (): boolean => registry.size > 0;
