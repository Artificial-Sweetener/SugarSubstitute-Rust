import {
  defineComponent,
  h,
  onBeforeUnmount,
  onMounted,
  ref,
  watch
} from "./chunk-3W5PGMRW.js";

// frontend/comfyui/ui/model_library/ModelTitleMarquee.ts
function modelTitleMarquee(elapsedMs, overflowWidth) {
  const overflow = Math.max(0, overflowWidth);
  if (overflow === 0) return { phase: "start", offset: 0, leftFade: false, rightFade: false };
  const duration = Math.max(1, Math.round(overflow / 44 * 1e3));
  const position = Math.max(0, elapsedMs) % (900 + duration + 700);
  if (position < 900) return { phase: "start", offset: 0, leftFade: false, rightFade: true };
  if (position - 900 < duration) {
    const offset = overflow * ((position - 900) / duration);
    return { phase: "scroll", offset, leftFade: offset > 0.5, rightFade: offset < overflow - 0.5 };
  }
  return { phase: "end", offset: overflow, leftFade: true, rightFade: false };
}

// frontend/comfyui/ui/model_library/vue/ModelGalleryTitle.ts
var ModelGalleryTitle = defineComponent({
  name: "SugarCubesModelGalleryTitle",
  props: { title: { type: String, required: true }, active: { type: Boolean, required: true } },
  setup(props) {
    const viewport = ref(null);
    const text = ref(null);
    const state = ref(null);
    let observer = null;
    let timer = null;
    let preference = null;
    let disposed = false;
    const stop = () => {
      if (timer !== null) clearInterval(timer);
      timer = null;
      state.value = null;
    };
    const restart = () => {
      stop();
      if (disposed || !props.active || preference?.matches || !viewport.value || !text.value)
        return;
      const overflow = text.value.getBoundingClientRect().width - viewport.value.clientWidth;
      if (overflow <= 0) return;
      const started = performance.now();
      state.value = modelTitleMarquee(0, overflow);
      timer = setInterval(() => {
        state.value = modelTitleMarquee(performance.now() - started, overflow);
      }, 33);
    };
    onMounted(() => {
      preference = window.matchMedia?.("(prefers-reduced-motion: reduce)") ?? null;
      preference?.addEventListener("change", restart);
      observer = new ResizeObserver(restart);
      if (viewport.value) observer.observe(viewport.value);
      document.fonts?.addEventListener("loadingdone", restart);
      restart();
    });
    watch(() => [props.active, props.title], restart, { flush: "post" });
    onBeforeUnmount(() => {
      disposed = true;
      stop();
      observer?.disconnect();
      preference?.removeEventListener("change", restart);
      document.fonts?.removeEventListener("loadingdone", restart);
    });
    return () => {
      const motion = state.value;
      return h(
        "span",
        {
          ref: viewport,
          class: "sc-model-title",
          "data-marquee-phase": motion?.phase ?? "static",
          "data-marquee-left-fade": motion?.leftFade ?? false,
          "data-marquee-right-fade": motion?.rightFade ?? false
        },
        h(
          "span",
          {
            ref: text,
            class: "sc-model-marquee-text",
            dir: "ltr",
            style: motion?.phase === "scroll" ? { transform: `translateX(-${String(motion.offset)}px)` } : void 0
          },
          props.title
        )
      );
    };
  }
});

export {
  ModelGalleryTitle
};
