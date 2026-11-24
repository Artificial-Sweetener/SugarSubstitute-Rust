// frontend/comfyui/ui/prompt_editor/runtime/PromptSearchPainter.ts
var PromptSearchPainter = class {
  /** Bind one editor; the masonry already shows the prompt, so a reveal is ready at once. */
  constructor(element, api, report) {
    this.element = element;
    this.api = api;
    this.report = report;
    api.setSearchRevealAdapter({ reveal: () => Promise.resolve("ready") });
  }
  #request = null;
  #published = null;
  #painted = null;
  #query = null;
  #queries = 0;
  #results = 0;
  paint(query, ranges, active) {
    if (this.#query !== query) this.#request = null;
    this.#query = query;
    this.#painted = { ranges, active };
    if (this.#publish(ranges, active) === "stale") {
      this.#request = null;
      this.#publish(ranges, active);
    }
  }
  reveal(index) {
    void this.#navigate(index, false).catch(this.report);
  }
  focus(index) {
    const query = this.#query;
    void this.#navigate(index, true).catch(this.report).finally(() => {
      if (this.#query === query) this.clear();
    });
  }
  async #navigate(index, focus) {
    if (!this.#request) return;
    await this.api.navigateSearchMatch({
      request: this.#request,
      resultRevision: this.#results,
      matchId: String(index),
      select: true,
      scroll: true,
      focus
    });
  }
  /**
   * Paint again for the request the editor issues when it retires its matches. An edit changes
   * the source the ranges were found in, so those wait for the finder's next search; any other
   * retirement, such as a replaced model catalog, repaints the same ranges.
   */
  refresh(request) {
    if (this.#query === null || request === this.#published) return;
    const edited = this.#request?.sourceRevision !== request.sourceRevision;
    this.#request = request;
    if (!edited && this.#painted) this.#publish(this.#painted.ranges, this.#painted.active);
  }
  clear() {
    if (this.#query === null) return;
    this.#query = null;
    this.#request = null;
    this.#published = null;
    this.#painted = null;
    void this.api.clearSearch(false).catch(this.report);
  }
  #publish(ranges, active) {
    this.#request ??= this.api.setSearchContext({
      fieldId: "masonry-finder",
      queryId: `query-${String(++this.#queries)}`
    });
    if (!this.#request) return "unavailable";
    this.#published = this.#request;
    return this.api.publishSearchMatches({
      request: this.#request,
      resultRevision: ++this.#results,
      matches: ranges.map((range, index) => ({
        matchId: String(index),
        from: range.start,
        to: range.start + range.length
      })),
      activeIndex: active
    }).status;
  }
};

export {
  PromptSearchPainter
};
