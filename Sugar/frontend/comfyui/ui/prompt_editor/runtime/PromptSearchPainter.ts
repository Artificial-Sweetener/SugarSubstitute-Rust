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
/** Hand the finder's matches to the prompt editor, which ranges them over its own text. */
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
 * The editor knows where its text is drawn, so its ranges stay on their characters through
 * wrapping, scrolling and LoRA chips. A match is named by its index among the ranges last painted.
 */
export class PromptSearchPainter implements PromptSearchSurface {
  #request: SearchRequest | null = null;
  #published: SearchRequest | null = null;
  #painted: { ranges: readonly PromptSearchRange[]; active: number | null } | null = null;
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
    this.#painted = { ranges, active };
    // An edit retires the editor's request, so a refused publication is issued a new one.
    if (this.#publish(ranges, active) === 'stale') {
      this.#request = null;
      this.#publish(ranges, active);
    }
  }

  reveal(index: number): void {
    void this.#navigate(index, false).catch(this.report);
  }

  focus(index: number): void {
    const query = this.#query;
    void this.#navigate(index, true)
      .catch(this.report)
      // The marks stay until focus has arrived, because removing them cancels the focus.
      .finally(() => {
        if (this.#query === query) this.clear();
      });
  }

  async #navigate(index: number, focus: boolean): Promise<void> {
    if (!this.#request) return;
    await this.api.navigateSearchMatch({
      request: this.#request,
      resultRevision: this.#results,
      matchId: String(index),
      select: true,
      scroll: true,
      focus,
    });
  }

  /**
   * Paint again for the request the editor issues when it retires its matches. An edit changes
   * the source the ranges were found in, so those wait for the finder's next search; any other
   * retirement, such as a replaced model catalog, repaints the same ranges.
   */
  refresh(request: SearchRequest): void {
    if (this.#query === null || request === this.#published) return;
    const edited = this.#request?.sourceRevision !== request.sourceRevision;
    this.#request = request;
    if (!edited && this.#painted) this.#publish(this.#painted.ranges, this.#painted.active);
  }

  clear(): void {
    if (this.#query === null) return;
    this.#query = null;
    this.#request = null;
    this.#published = null;
    this.#painted = null;
    void this.api.clearSearch(false).catch(this.report);
  }

  #publish(ranges: readonly PromptSearchRange[], active: number | null): string {
    this.#request ??= this.api.setSearchContext({
      fieldId: 'masonry-finder',
      queryId: `query-${String(++this.#queries)}`,
    });
    if (!this.#request) return 'unavailable';
    this.#published = this.#request;
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
