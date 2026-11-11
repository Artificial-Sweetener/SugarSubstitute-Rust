import {
  defineComponent,
  h
} from "./chunk-3W5PGMRW.js";

// frontend/comfyui/ui/model_library/vue/ModelBusyRing.ts
var ModelBusyRing = defineComponent({
  name: "SugarCubesModelBusyRing",
  props: {
    size: { type: Number, default: 24 },
    strokeWidth: { type: Number, default: 6 }
  },
  setup(props) {
    return () => h(
      "svg",
      {
        class: "sc-model-busy-ring",
        viewBox: `0 0 ${props.size} ${props.size}`,
        style: { width: `${props.size}px`, height: `${props.size}px` },
        "aria-hidden": "true",
        focusable: "false"
      },
      [
        h("circle", {
          cx: props.size / 2,
          cy: props.size / 2,
          r: (props.size - props.strokeWidth) / 2,
          fill: "none",
          stroke: "currentColor",
          "stroke-width": props.strokeWidth,
          "stroke-linecap": "round",
          pathLength: 360,
          "stroke-dasharray": "105 255"
        })
      ]
    );
  }
});

export {
  ModelBusyRing
};
