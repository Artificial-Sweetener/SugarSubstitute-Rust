// frontend/sugarprompt/services/src/search/highlights.ts
var SEARCH_MATCH_HIGHLIGHT = "sc-search-match";
var SEARCH_CURRENT_HIGHLIGHT = "sc-search-current";
var SearchHighlightRanges = class {
  /** Bind the window whose highlight registry is shared; one without the API shows nothing. */
  constructor(view) {
    this.view = view;
  }
  #owned = [];
  /** Whether this window can show highlights at all. */
  get supported() {
    const view = this.view;
    return !!view?.Highlight && !!view.CSS?.highlights;
  }
  /** Replace this contributor's ranges, leaving every other contributor's in place. */
  replace(matches, current) {
    this.clear();
    const view = this.view;
    const registry = view?.CSS?.highlights;
    const Highlight = view?.Highlight;
    if (!registry || !Highlight) return;
    for (const [name, ranges] of [
      [SEARCH_MATCH_HIGHLIGHT, matches],
      [SEARCH_CURRENT_HIGHLIGHT, current]
    ]) {
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
  clear() {
    const registry = this.view?.CSS?.highlights;
    for (const { name, range } of this.#owned) {
      const highlight = registry?.get(name);
      highlight?.delete(range);
      if (highlight?.size === 0) registry?.delete(name);
    }
    this.#owned = [];
  }
};
export {
  SEARCH_CURRENT_HIGHLIGHT,
  SEARCH_MATCH_HIGHLIGHT,
  SearchHighlightRanges
};
