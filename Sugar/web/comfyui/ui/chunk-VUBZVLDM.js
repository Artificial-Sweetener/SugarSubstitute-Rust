import {
  CUBE_STACK_ROW_HEIGHT,
  cubeStackRowStride
} from "./chunk-D6BK5OR7.js";
import {
  defineComponent,
  h
} from "./chunk-6PAJ5Y37.js";

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
  CubeStackSelectionIndicator
};
