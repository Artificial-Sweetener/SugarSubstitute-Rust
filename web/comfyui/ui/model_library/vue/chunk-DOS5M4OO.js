import {
  ModelRecommendationPreview
} from "./chunk-MRDNZD2T.js";
import {
  defineComponent,
  h
} from "./chunk-57LGDI7T.js";

// frontend/comfyui/ui/model_library/vue/ModelDiscoveryPortrait.ts
var ModelDiscoveryPortrait = defineComponent({
  name: "SugarCubesModelDiscoveryPortrait",
  props: {
    previewUrl: { type: String, default: null },
    title: { type: String, required: true },
    selected: { type: Boolean, required: true },
    disabled: { type: Boolean, required: true }
  },
  emits: { select: () => true },
  setup(props, { emit }) {
    return () => {
      return h(
        "button",
        {
          type: "button",
          class: ["sc-model-discovery-portrait", { "is-selected": props.selected }],
          role: "checkbox",
          "aria-checked": props.selected,
          "aria-label": props.title,
          disabled: props.disabled,
          onClick: () => emit("select")
        },
        [
          h(ModelRecommendationPreview, { url: props.previewUrl }),
          h(
            "span",
            { class: "sc-model-discovery-caption" },
            props.title.split("|").flatMap(
              (part, index, parts) => index === parts.length - 1 ? [part] : [part + "|", h("wbr")]
            )
          ),
          h(
            "span",
            { class: "sc-model-discovery-check", "aria-hidden": "true" },
            props.selected ? "\u2713" : ""
          )
        ]
      );
    };
  }
});

export {
  ModelDiscoveryPortrait
};
