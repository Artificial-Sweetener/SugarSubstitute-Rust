import {
  defineComponent,
  h
} from "./chunk-6PAJ5Y37.js";

// frontend/comfyui/ui/output_canvas/presentation/vue/CanvasComparisonExit.ts
var CanvasComparisonExit = defineComponent({
  name: "CanvasComparisonExit",
  props: {
    send: { type: Function, required: true }
  },
  /** Preserve a direct exit while resources fail or a host renegotiates. */
  setup(props) {
    return () => h(
      "button",
      {
        type: "button",
        class: "sc-output-segment sc-output-comparison-exit",
        "data-sc-fixed-navigation": "true",
        "aria-label": "Exit comparison",
        title: "Exit comparison",
        onClick: () => props.send({ type: "comparison", enabled: false })
      },
      "Exit"
    );
  }
});

export {
  CanvasComparisonExit
};
