// frontend/comfyui/ui/prompt_editor/runtime/PromptCompletionCatalog.ts
var PROMPT_LIBRARY_REVISION = "library";
var PromptCompletionCatalog = class {
  #current = null;
  #held = null;
  /** Search `next` from now on; the returned catalog is the same object unless authority changed. */
  follow(next) {
    this.#current = next;
    if (!next) this.#held = null;
    else if (this.#held?.authoritative !== next.authoritative)
      this.#held = Object.freeze({
        revision: PROMPT_LIBRARY_REVISION,
        authoritative: next.authoritative,
        search: (query) => this.#current?.search(query) ?? []
      });
    return this.#held;
  }
};

export {
  PROMPT_LIBRARY_REVISION,
  PromptCompletionCatalog
};
