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
  #query = null;
  #queries = 0;
  #results = 0;
  paint(query, ranges, active) {
    if (this.#query !== query) this.#request = null;
    this.#query = query;
    if (this.#publish(ranges, active) === "stale") {
      this.#request = null;
      this.#publish(ranges, active);
    }
  }
  reveal(index, focus) {
    if (!this.#request) return;
    void this.api.navigateSearchMatch({
      request: this.#request,
      resultRevision: this.#results,
      matchId: String(index),
      select: true,
      scroll: true,
      focus
    }).catch(this.report);
  }
  clear() {
    if (this.#query === null) return;
    this.#query = null;
    this.#request = null;
    void this.api.clearSearch(false).catch(this.report);
  }
  #publish(ranges, active) {
    this.#request ??= this.api.setSearchContext({
      fieldId: "masonry-finder",
      queryId: `query-${String(++this.#queries)}`
    });
    if (!this.#request) return "unavailable";
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
