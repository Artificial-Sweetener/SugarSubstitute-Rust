import {
  onBeforeUnmount,
  shallowRef,
  watch
} from "./chunk-OJTDXCRK.js";

// frontend/comfyui/ui/model_library/ModelImagePresentation.ts
var DECODED_MODEL_IMAGES = 512;
var decoded = /* @__PURE__ */ new Map();
var decoding = /* @__PURE__ */ new Map();
function modelImageDecoded(url) {
  return decoded.has(url);
}
var decodeModelImage = (url) => {
  const kept = decoded.get(url);
  if (kept) {
    decoded.delete(url);
    decoded.set(url, kept);
    return Promise.resolve(true);
  }
  const running = decoding.get(url);
  if (running) return running;
  const image = new Image();
  image.decoding = "async";
  image.referrerPolicy = "no-referrer";
  image.src = url;
  const settled = image.decode().then(
    () => {
      decoded.set(url, image);
      for (const oldest of decoded.keys()) {
        if (decoded.size <= DECODED_MODEL_IMAGES) break;
        decoded.delete(oldest);
      }
      return true;
    },
    () => false
  );
  decoding.set(url, settled);
  void settled.then(() => decoding.delete(url));
  return settled;
};
var NEVER = Symbol("never failed");
var ModelImagePresentation = class {
  constructor(changed, decode = decodeModelImage, ready = modelImageDecoded) {
    this.changed = changed;
    this.decode = decode;
    this.ready = ready;
  }
  #shown = null;
  #wanted = null;
  #failed = NEVER;
  #pending = false;
  #attempt = 0;
  #disposed = false;
  /** The address to draw, or null while there is none to draw. */
  get shown() {
    return this.#shown;
  }
  /** Whether an image is being decoded with nothing on screen to keep meanwhile. */
  get loading() {
    return this.#pending && this.#shown === null;
  }
  /** Whether there is nothing to draw because the wanted image cannot be shown. */
  get unavailable() {
    return this.#shown === null && !this.#pending;
  }
  /**
   * Ask for `url`, which came from `record`. Asking again for an address that failed does nothing
   * until it comes from a different record.
   */
  show(url, record = url) {
    if (this.#disposed) return;
    if (url === this.#wanted && (this.#failed === NEVER || this.#failed === record)) return;
    this.#wanted = url;
    this.#failed = NEVER;
    const attempt = ++this.#attempt;
    if (url === null) {
      this.#pending = false;
      this.#shown = null;
      this.changed();
      return;
    }
    if (this.ready(url)) {
      this.#pending = false;
      this.#shown = url;
      this.changed();
      return;
    }
    this.#pending = true;
    this.changed();
    void this.decode(url).then((shown) => {
      if (this.#disposed || attempt !== this.#attempt) return;
      this.#pending = false;
      if (shown) this.#shown = url;
      else this.#failed = record;
      this.changed();
    });
  }
  /** Stop reporting; a decode still running keeps its image for the next surface that asks. */
  dispose() {
    this.#disposed = true;
  }
};

// frontend/comfyui/ui/model_library/vue/ModelImage.ts
function useModelImage(url, record = url) {
  const drawn = shallowRef(0);
  const presentation = new ModelImagePresentation(() => {
    drawn.value++;
  });
  watch([url, record], ([wanted, from]) => presentation.show(wanted, from), {
    immediate: true,
    flush: "sync"
  });
  onBeforeUnmount(() => presentation.dispose());
  return () => {
    void drawn.value;
    return {
      url: presentation.shown,
      loading: presentation.loading,
      unavailable: presentation.unavailable
    };
  };
}

export {
  useModelImage
};
