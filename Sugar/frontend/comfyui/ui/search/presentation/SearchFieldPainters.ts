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
/** Hand each prompt's matches to the prompt, which paints them in its own text. */
import type { SearchMatch } from '../domain/SearchMatching.js';
import type { PromptSearchSurface } from '../../prompt_editor/PromptSearchSurface.js';
import type { SearchFieldSurface } from './SearchSurface.js';

/** Own which prompts currently show matches, so each is cleared exactly once. */
export class SearchFieldPainters {
  readonly #painted = new Map<string, PromptSearchSurface>();
  #matches: readonly SearchMatch[] = [];
  #query = '';

  /** Paint every prompt's matches for `query` and clear prompts that no longer match. */
  render(
    fields: ReadonlyMap<string, SearchFieldSurface>,
    matches: readonly SearchMatch[],
    active: SearchMatch | null,
    query: string,
  ): void {
    this.#matches = matches;
    this.#query = query;
    const matched = new Set(matches.map((match) => match.field));
    for (const [id, painter] of this.#painted)
      if (!matched.has(id) || fields.get(id)?.painter !== painter) {
        painter.clear();
        this.#painted.delete(id);
      }
    for (const id of matched) {
      const painter = fields.get(id)?.painter;
      if (!painter) continue;
      this.#painted.set(id, painter);
      this.#paint(id, painter, active);
    }
  }

  /** Move the stronger current-match colour between prompts without recomputing matches. */
  activate(previous: SearchMatch | null, active: SearchMatch | null): void {
    for (const id of new Set([previous?.field, active?.field])) {
      const painter = id === undefined ? undefined : this.#painted.get(id);
      if (id !== undefined && painter) this.#paint(id, painter, active);
    }
  }

  /** Select and scroll to one match inside its prompt; false when another renderer owns it. */
  reveal(match: SearchMatch): boolean {
    const painter = this.#painted.get(match.field);
    if (!painter) return false;
    painter.reveal(this.#index(match));
    return true;
  }

  /**
   * Remove every prompt's matches. With `focus`, the prompt holding that match first takes
   * focus with the match selected; the return says whether a prompt did.
   */
  clear(focus: SearchMatch | null = null): boolean {
    const focused = focus ? this.#painted.get(focus.field) : undefined;
    if (focus && focused) focused.focus(this.#index(focus));
    for (const painter of this.#painted.values()) if (painter !== focused) painter.clear();
    this.#painted.clear();
    this.#matches = [];
    return focused !== undefined;
  }

  #within(field: string): SearchMatch[] {
    return this.#matches.filter((match) => match.field === field);
  }

  /** Where a match stands among its own field's matches. */
  #index(match: SearchMatch): number {
    return this.#within(match.field).findIndex(
      (candidate) => candidate.start === match.start && candidate.length === match.length,
    );
  }

  #paint(field: string, painter: PromptSearchSurface, active: SearchMatch | null): void {
    const index = active?.field === field ? this.#index(active) : -1;
    painter.paint(this.#query, this.#within(field), index < 0 ? null : index);
  }
}
