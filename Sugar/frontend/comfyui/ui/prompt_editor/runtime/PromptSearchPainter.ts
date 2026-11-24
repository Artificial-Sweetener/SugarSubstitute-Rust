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
/** Paint the finder's matches through the prompt editor's own search layer. */
import type { PromptEditorApi } from '../../../../sugarprompt/editor/src/contracts.js';
import type { SearchRequest } from '../../../../sugarprompt/services/src/search/contracts.js';
import type { PromptSearchRange, PromptSearchSurface } from '../PromptSearchSurface.js';

type SearchApi = Pick<
  PromptEditorApi,
  | 'setSearchContext'
  | 'publishSearchMatches'
  | 'navigateSearchMatch'
  | 'clearSearch'
  | 'setSearchRevealAdapter'
>;

/**
 * The editor measures its matches itself, so they stay on their characters through wrapping,
 * scrolling and LoRA chips. A match is named by its index among the ranges last painted.
 */
export class PromptSearchPainter implements PromptSearchSurface {
  #request: SearchRequest | null = null;
  #query: string | null = null;
  #queries = 0;
  #results = 0;

  /** Bind one editor; the masonry already shows the prompt, so a reveal is ready at once. */
  constructor(
    readonly element: HTMLElement,
    private readonly api: SearchApi,
    private readonly report: (cause: unknown) => void,
  ) {
    api.setSearchRevealAdapter({ reveal: () => Promise.resolve('ready') });
  }

  paint(query: string, ranges: readonly PromptSearchRange[], active: number | null): void {
    if (this.#query !== query) this.#request = null;
    this.#query = query;
    // An edit retires the editor's request, so a refused publication is issued a new one.
    if (this.#publish(ranges, active) === 'stale') {
      this.#request = null;
      this.#publish(ranges, active);
    }
  }

  reveal(index: number, focus: boolean): void {
    if (!this.#request) return;
    void this.api
      .navigateSearchMatch({
        request: this.#request,
        resultRevision: this.#results,
        matchId: String(index),
        select: true,
        scroll: true,
        focus,
      })
      .catch(this.report);
  }

  clear(): void {
    if (this.#query === null) return;
    this.#query = null;
    this.#request = null;
    void this.api.clearSearch(false).catch(this.report);
  }

  #publish(ranges: readonly PromptSearchRange[], active: number | null): string {
    this.#request ??= this.api.setSearchContext({
      fieldId: 'masonry-finder',
      queryId: `query-${String(++this.#queries)}`,
    });
    if (!this.#request) return 'unavailable';
    return this.api.publishSearchMatches({
      request: this.#request,
      resultRevision: ++this.#results,
      matches: ranges.map((range, index) => ({
        matchId: String(index),
        from: range.start,
        to: range.start + range.length,
      })),
      activeIndex: active,
    }).status;
  }
}
