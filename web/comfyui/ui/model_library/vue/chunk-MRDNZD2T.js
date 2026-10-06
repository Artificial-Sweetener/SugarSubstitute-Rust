import {
  ModelBusyRing
} from "./chunk-3UMXK5CY.js";
import {
  defineComponent,
  h,
  ref,
  watch
} from "./chunk-57LGDI7T.js";

// frontend/comfyui/ui/model_library/vue/ModelRecommendationPreview.ts
var ModelRecommendationPreview = defineComponent({
  name: "SugarCubesModelRecommendationPreview",
  props: {
    url: { type: String, default: null }
  },
  setup(props) {
    const status = ref("unavailable");
    let revision = 0;
    watch(
      () => props.url,
      (url) => {
        revision++;
        status.value = url ? "loading" : "unavailable";
      },
      { immediate: true, flush: "sync" }
    );
    return () => {
      const url = props.url;
      const renderedRevision = revision;
      return h("span", { class: "sc-model-recommendation-preview" }, [
        url && status.value !== "unavailable" ? h("img", {
          key: url,
          src: url,
          alt: "",
          draggable: false,
          decoding: "async",
          onLoad: () => {
            if (renderedRevision === revision && status.value === "loading")
              status.value = "ready";
          },
          onError: () => {
            if (renderedRevision === revision && status.value === "loading")
              status.value = "unavailable";
          }
        }) : h("span", { class: "sc-model-preview-unavailable" }, "Preview unavailable"),
        status.value === "loading" ? h("span", { class: "sc-model-preview-loading", role: "status" }, [
          h(ModelBusyRing, { class: "sc-model-preview-busy-ring", size: 34, strokeWidth: 4 }),
          h("span", { class: "sc-model-preview-loading-label" }, "Loading preview\u2026")
        ]) : null
      ]);
    };
  }
});

export {
  ModelRecommendationPreview
};
