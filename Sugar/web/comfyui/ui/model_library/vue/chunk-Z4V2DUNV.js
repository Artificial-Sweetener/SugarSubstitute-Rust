import {
  ModelBusyRing
} from "./chunk-ZVGMUCWU.js";
import {
  useModelImage
} from "./chunk-SPVIVHMR.js";
import {
  defineComponent,
  h
} from "./chunk-OJTDXCRK.js";

// frontend/comfyui/ui/model_library/vue/ModelRecommendationPreview.ts
var ModelRecommendationPreview = defineComponent({
  name: "SugarCubesModelRecommendationPreview",
  props: {
    url: { type: String, default: null }
  },
  setup(props) {
    const image = useModelImage(() => props.url);
    return () => {
      const { url, loading, unavailable } = image();
      return h("span", { class: "sc-model-recommendation-preview" }, [
        url ? h("img", { src: url, alt: "", draggable: false }) : unavailable ? h("span", { class: "sc-model-preview-unavailable" }, "Preview unavailable") : null,
        loading ? h("span", { class: "sc-model-preview-loading", role: "status" }, [
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
