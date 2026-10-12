import {
  defineComponent,
  h,
  onBeforeUnmount,
  ref,
  watch
} from "./chunk-MNPWJMUZ.js";

// frontend/comfyui/ui/output_canvas/presentation/vue/CanvasLoadingIndicator.ts
var CanvasLoadingIndicator = defineComponent({
  name: "CanvasLoadingIndicator",
  props: { loading: { type: Boolean, required: true } },
  /** Cancel delayed feedback on completion and attachment retirement. */
  setup(props) {
    const visible = ref(false);
    let timer = null;
    const clear = () => {
      if (timer !== null) clearTimeout(timer);
      timer = null;
      visible.value = false;
    };
    watch(
      () => props.loading,
      (loading) => {
        clear();
        if (loading)
          timer = setTimeout(() => {
            timer = null;
            visible.value = true;
          }, 250);
      },
      { immediate: true }
    );
    onBeforeUnmount(clear);
    return () => visible.value ? h(
      "div",
      {
        class: "sc-output-loading",
        role: "status",
        "aria-label": "Loading workflow image"
      },
      [h("span", { class: "sc-output-spinner", "aria-hidden": "true" })]
    ) : null;
  }
});

export {
  CanvasLoadingIndicator
};
