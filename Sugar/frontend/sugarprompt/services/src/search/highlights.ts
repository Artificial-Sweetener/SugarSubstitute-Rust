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
/**
 * The one way search matches are shown: the browser's own text highlighting, through the CSS
 * Custom Highlight API. Every renderer of searchable text (a prompt editor, a text box's mirror,
 * a value a card renders) contributes ranges over its own text nodes to the document's two
 * highlights, and the host styles those two names once:
 * - the current match with the declarations of its `::selection`, so it is drawn as a selection;
 * - every other match in a colour derived from the accent.
 */

/** Every match that is not the current one. */
export const SEARCH_MATCH_HIGHLIGHT = 'sc-search-match';
/** The current match. */
export const SEARCH_CURRENT_HIGHLIGHT = 'sc-search-current';

interface HighlightSet {
  add(range: Range): unknown;
  delete(range: Range): unknown;
  readonly size: number;
}
interface HighlightWindow {
  readonly Highlight?: new (...ranges: Range[]) => HighlightSet;
  readonly CSS?: { readonly highlights?: Map<string, HighlightSet> };
}

/** One contributor's ranges within the document's shared search highlights. */
export class SearchHighlightRanges {
  #owned: { readonly name: string; readonly range: Range }[] = [];
  private readonly view: (Window & typeof globalThis) | null;

  /** Bind the window whose highlight registry is shared; one without the API shows nothing. */
  constructor(view: (Window & typeof globalThis) | null) {
    this.view = view;
  }

  /** Whether this window can show highlights at all. */
  get supported(): boolean {
    const view = this.view as HighlightWindow | null;
    return !!view?.Highlight && !!view.CSS?.highlights;
  }

  /** Replace this contributor's ranges, leaving every other contributor's in place. */
  replace(matches: readonly Range[], current: readonly Range[]): void {
    this.clear();
    const view = this.view as HighlightWindow | null;
    const registry = view?.CSS?.highlights;
    const Highlight = view?.Highlight;
    if (!registry || !Highlight) return;
    for (const [name, ranges] of [
      [SEARCH_MATCH_HIGHLIGHT, matches],
      [SEARCH_CURRENT_HIGHLIGHT, current],
    ] as const) {
      if (ranges.length === 0) continue;
      let highlight = registry.get(name);
      if (!highlight) {
        highlight = new Highlight();
        registry.set(name, highlight);
      }
      for (const range of ranges) {
        highlight.add(range);
        this.#owned.push({ name, range });
      }
    }
  }

  /** Remove this contributor's ranges; a highlight left empty leaves the registry. */
  clear(): void {
    const registry = (this.view as HighlightWindow | null)?.CSS?.highlights;
    for (const { name, range } of this.#owned) {
      const highlight = registry?.get(name);
      highlight?.delete(range);
      if (highlight?.size === 0) registry?.delete(name);
    }
    this.#owned = [];
  }
}
