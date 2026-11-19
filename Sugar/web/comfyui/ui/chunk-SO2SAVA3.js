import {
  defineComponent,
  h
} from "./chunk-6PAJ5Y37.js";

// frontend/comfyui/ui/surface/stack/CubeStackMetrics.ts
var CUBE_STACK_ROW_HEIGHT = 46;
var CUBE_STACK_ROW_GAP = 4;
function cubeStackRowGap(packed) {
  return packed ? 0 : CUBE_STACK_ROW_GAP;
}
function cubeStackRowStride(packed) {
  return CUBE_STACK_ROW_HEIGHT + cubeStackRowGap(packed);
}

// frontend/comfyui/ui/surface/stack/vue/CubeStackSelectionIndicator.ts
var CubeStackSelectionIndicator = defineComponent({
  props: {
    position: { type: Number, default: null },
    packed: { type: Boolean, default: false }
  },
  setup(props) {
    return () => h("div", {
      class: "sc-cube-stack__selection",
      "aria-hidden": "true",
      "data-position": props.position ?? void 0,
      style: {
        opacity: props.position === null ? "0" : "1",
        transform: `translateY(${String((props.position ?? 0) * cubeStackRowStride(props.packed) + (CUBE_STACK_ROW_HEIGHT - 16) / 2)}px)`
      }
    });
  }
});

export {
  CUBE_STACK_ROW_HEIGHT,
  cubeStackRowGap,
  cubeStackRowStride,
  CubeStackSelectionIndicator
};
