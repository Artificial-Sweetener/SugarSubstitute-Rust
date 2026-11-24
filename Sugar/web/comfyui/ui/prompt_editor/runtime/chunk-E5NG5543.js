// frontend/comfyui/ui/prompt_editor/runtime/PromptCatalogAdmission.ts
var PromptCatalogAdmission = class {
  #element;
  #target;
  #window;
  #menuOpen = false;
  #pointerHeld = false;
  #catalog = null;
  #media = false;
  #admitted = false;
  /** Watch the prompt's own focus and pointer; the menu's owner reports the menu. */
  constructor(element, target) {
    this.#element = element;
    this.#target = target;
    this.#window = element.ownerDocument.defaultView;
    element.addEventListener("pointerdown", this.#pressed, true);
    element.addEventListener("focusout", this.#released);
    this.#window?.addEventListener("pointerup", this.#released, true);
    this.#window?.addEventListener("pointercancel", this.#released, true);
  }
  /** Follow the prompt's menu, which takes focus out of the prompt while it is open. */
  menuPresented(open) {
    this.#menuOpen = open;
    if (!open) this.#settleSoon();
  }
  /** Apply a catalog now, or keep the newest one until the prompt is free. */
  offerCatalog(adapter) {
    if (!this.#admitted || !this.#busy(true)) this.#applyCatalog(adapter);
    else this.#catalog = adapter;
  }
  /** Refresh chip images now, or once the menu is closed and the pointer released. */
  offerMedia() {
    if (this.#busy(false)) this.#media = true;
    else this.#target.media();
  }
  /** Stop watching; nothing held is applied to a disposed editor. */
  dispose() {
    this.#element.removeEventListener("pointerdown", this.#pressed, true);
    this.#element.removeEventListener("focusout", this.#released);
    this.#window?.removeEventListener("pointerup", this.#released, true);
    this.#window?.removeEventListener("pointercancel", this.#released, true);
    this.#catalog = null;
    this.#media = false;
  }
  #pressed = () => {
    this.#pointerHeld = true;
  };
  #released = (event) => {
    if (event.type !== "focusout") this.#pointerHeld = false;
    this.#settleSoon();
  };
  /** Settle after the event that freed the prompt has finished moving focus. */
  #settleSoon() {
    queueMicrotask(() => this.#settle());
  }
  #settle() {
    if (this.#catalog && !this.#busy(true)) this.#applyCatalog(this.#catalog);
    else if (this.#media && !this.#busy(false)) {
      this.#media = false;
      this.#target.media();
    }
  }
  /** A new catalog carries its own images, so held images are covered by it. */
  #applyCatalog(adapter) {
    this.#admitted = true;
    this.#catalog = null;
    this.#media = false;
    this.#target.catalog(adapter);
  }
  #busy(focusCounts) {
    return this.#menuOpen || this.#pointerHeld || focusCounts && this.#element.contains(this.#element.ownerDocument.activeElement);
  }
};

export {
  PromptCatalogAdmission
};
