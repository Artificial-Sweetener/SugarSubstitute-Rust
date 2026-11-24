import {
  onBeforeUnmount,
  shallowRef
} from "./chunk-ZMBAFDHL.js";

// frontend/comfyui/ui/interaction/reorder/ReorderGesture.ts
var ReorderGesture = class {
  constructor(options) {
    this.options = options;
  }
  #candidate = null;
  #card = null;
  #order = null;
  #target = null;
  #suppressClick = false;
  #frame = 0;
  #x = 0;
  #y = 0;
  /** The card in the air, or null. */
  get card() {
    return this.#card;
  }
  /** The order being previewed, or null while the accepted order shows. */
  get order() {
    return this.#order;
  }
  /** Whether a press is held, lifted or not. */
  get pressed() {
    return this.#candidate !== null;
  }
  /** Remember a press on `item`. Returns false when the press cannot become a drag. */
  press(event, key, item) {
    if (this.#candidate || event.button !== 0 || this.options.admission(key) === "refuse")
      return false;
    const bounds = item.getBoundingClientRect();
    if (!this.options.propagatePress) event.stopPropagation();
    this.#candidate = {
      key,
      pointerId: event.pointerId,
      startX: event.clientX,
      startY: event.clientY,
      offsetX: event.clientX - bounds.left,
      offsetY: event.clientY - bounds.top,
      width: bounds.width,
      height: bounds.height,
      original: this.options.keys()
    };
    this.#suppressClick = false;
    this.#x = event.clientX;
    this.#y = event.clientY;
    const { document } = this.options;
    document.addEventListener("keydown", this.#escape, true);
    document.addEventListener("pointermove", this.#move, true);
    document.addEventListener("pointerup", this.#finish, true);
    document.defaultView?.addEventListener("blur", this.#blur);
    if (this.options.capture === "press")
      this.options.captureElement()?.setPointerCapture(event.pointerId);
    return true;
  }
  /** Choose the slot again after the list moved under a lifted card, as when it scrolls. */
  reproject() {
    this.#project();
  }
  /** Lift a held press now that its caller is free, or follow the pointer of a lifted card. */
  resume() {
    const active = this.#candidate;
    if (!active) return;
    const admission = this.options.admission(active.key);
    if (admission === "defer") return;
    if (!this.#card && Math.hypot(this.#x - active.startX, this.#y - active.startY) < this.options.threshold)
      return;
    if (admission === "refuse") {
      this.clear();
      return;
    }
    const lifting = !this.#card;
    if (lifting) {
      active.original = this.options.keys();
      if (this.options.capture !== "press")
        this.options.captureElement()?.setPointerCapture(active.pointerId);
    }
    this.#card = {
      key: active.key,
      x: this.#x - active.offsetX,
      y: this.#y - active.offsetY,
      width: active.width,
      height: active.height,
      grabX: active.offsetX,
      grabY: active.offsetY
    };
    this.#suppressClick = true;
    this.options.moved(this.#card);
    if (!this.#order) {
      this.#order = active.original;
      this.options.lifted(active.key);
      this.#frame = this.#window()?.requestAnimationFrame(this.#edgeScroll) ?? 0;
    }
    this.#project();
  }
  /** Abandon the drag: a lost pointer, a list that changed under it, a caller going away. */
  cancel = () => {
    if (!this.#candidate) return;
    this.clear();
    this.#suppressClick = false;
  };
  /** End the gesture. `abandoned` tells the caller a lifted card was not dropped. */
  clear(abandoned = true) {
    const previous = this.#candidate;
    const lifted = this.#card !== null;
    this.#candidate = null;
    const view = this.#window();
    view?.cancelAnimationFrame(this.#frame);
    this.#frame = 0;
    const holder = this.options.captureElement();
    if (previous && holder?.hasPointerCapture?.(previous.pointerId))
      holder.releasePointerCapture(previous.pointerId);
    const { document } = this.options;
    document.removeEventListener("keydown", this.#escape, true);
    document.removeEventListener("pointermove", this.#move, true);
    document.removeEventListener("pointerup", this.#finish, true);
    view?.removeEventListener("blur", this.#blur);
    this.#withdrawTarget(previous?.key ?? "");
    if (abandoned && lifted) {
      this.options.cancelled(true);
      this.options.preview(null);
    } else if (abandoned && previous) this.options.cancelled(false);
    this.#card = null;
    this.#order = null;
    if (lifted) this.options.moved(null);
  }
  /** A new press anywhere in the list begins with no drag to forget. */
  beginInteraction = () => {
    if (!this.#candidate) this.#suppressClick = false;
  };
  /** Swallow the click a browser fires when a dragged pointer is released. */
  click = (event) => {
    if (!this.#suppressClick || event.detail === 0) return;
    event.preventDefault();
    event.stopPropagation();
    this.#suppressClick = false;
  };
  #blur = () => this.cancel();
  #escape = (event) => {
    if (event.key !== "Escape") return;
    event.preventDefault();
    event.stopPropagation();
    this.clear();
  };
  #move = (event) => {
    const active = this.#candidate;
    if (!active || event.pointerId !== active.pointerId) return;
    event.stopPropagation();
    this.#x = event.clientX;
    this.#y = event.clientY;
    if (Math.hypot(this.#x - active.startX, this.#y - active.startY) >= this.options.threshold) {
      event.preventDefault();
      this.#suppressClick = true;
    }
    this.resume();
  };
  #finish = (event) => {
    const active = this.#candidate;
    if (!active || event.pointerId !== active.pointerId) return;
    event.stopPropagation();
    const target = this.#card ? this.#target : null;
    if (target) {
      const { owner, hit } = target;
      this.#target = null;
      this.clear(false);
      this.options.preview(null);
      owner.show(active.key, null);
      owner.accept(active.key, hit);
      return;
    }
    const final = this.#order;
    const inside = this.options.accepts?.(event.clientX, event.clientY) ?? true;
    if (!inside || !final || !this.#card || final.every((key, index) => key === active.original[index])) {
      this.clear();
      return;
    }
    this.clear(false);
    this.options.commit(active.key, final);
  };
  /** Choose the slot under the lifted card, or hand the preview to a drop target it is over. */
  #project() {
    const active = this.#candidate;
    const card = this.#card;
    if (!active || !card) return;
    for (const owner of this.options.targets ?? []) {
      const hit = owner.hit(this.#x, this.#y, active.key);
      if (hit === null || hit === void 0) continue;
      if (this.#target && this.#target.owner !== owner) this.#withdrawTarget(active.key);
      if (!this.#target && !this.#same(this.#order, active.original)) {
        this.#order = active.original;
        this.options.preview(active.original);
      }
      this.#target = { owner, hit };
      owner.show(active.key, hit);
      return;
    }
    this.#withdrawTarget(active.key);
    if (this.options.projects && !this.options.projects(this.#x, this.#y)) return;
    const projected = this.options.project(card, this.#order ?? active.original);
    if (!projected) return;
    this.#order = projected;
    this.options.preview(projected);
  }
  #withdrawTarget(key) {
    if (!this.#target) return;
    this.#target.owner.show(key, null);
    this.#target = null;
  }
  #same(left, right) {
    return left !== null && left.length === right.length && left.every((key, i) => key === right[i]);
  }
  /** Keep scrolling while the lifted card rests near the list's leading or trailing edge. */
  #edgeScroll = () => {
    this.#frame = 0;
    const scroll = this.options.scroll;
    const viewport = scroll?.viewport() ?? null;
    if (!this.#card) return;
    if (scroll && viewport) {
      const bounds = viewport.getBoundingClientRect();
      const row = scroll.axis === "x";
      const [along, start, end] = row ? [this.#x, bounds.left, bounds.right] : [this.#y, bounds.top, bounds.bottom];
      const [across, near, far] = row ? [this.#y, bounds.top, bounds.bottom] : [this.#x, bounds.left, bounds.right];
      const delta = along < start + scroll.edge ? -scroll.step : along > end - scroll.edge ? scroll.step : 0;
      if (delta && across >= near - scroll.lateral && across <= far + scroll.lateral) {
        const before = row ? viewport.scrollLeft : viewport.scrollTop;
        if (row)
          viewport.scrollLeft = Math.max(
            0,
            Math.min(viewport.scrollWidth - viewport.clientWidth, before + delta)
          );
        else viewport.scrollTop += delta;
        if ((row ? viewport.scrollLeft : viewport.scrollTop) !== before || !row) this.#project();
      }
    }
    this.#frame = this.#window()?.requestAnimationFrame(this.#edgeScroll) ?? 0;
  };
  #window() {
    return this.options.document.defaultView;
  }
};

// frontend/comfyui/ui/interaction/reorder/vue/useReorderGesture.ts
function useReorderGesture(setup) {
  const card = shallowRef(null);
  const order = shallowRef(null);
  let gesture = null;
  const made = () => {
    const root = setup.root.value;
    if (gesture || !root) return gesture;
    gesture = new ReorderGesture({
      ...setup,
      document: root.ownerDocument,
      captureElement: () => setup.root.value,
      moved: (next) => {
        card.value = next;
        order.value = next ? gesture?.order ?? order.value : null;
      },
      preview: (next) => {
        order.value = next ?? (card.value ? gesture?.order ?? null : null);
        setup.preview(next);
      }
    });
    return gesture;
  };
  onBeforeUnmount(() => gesture?.clear());
  return {
    card,
    order,
    press: (event, key) => {
      const item = event.currentTarget;
      if (item instanceof HTMLElement) made()?.press(event, key, item);
    },
    resume: () => gesture?.resume(),
    cancel: () => gesture?.cancel(),
    clear: () => gesture?.clear(),
    click: (event) => gesture?.click(event),
    beginInteraction: () => gesture?.beginInteraction()
  };
}

export {
  useReorderGesture
};
