import {
  CanvasRoutePicker
} from "./chunk-P7TVKFNT.js";
import {
  CanvasSelectionPicker
} from "./chunk-KKVBN3OP.js";
import {
  CanvasComparisonExit
} from "./chunk-J2E7XMH3.js";
import {
  defineComponent,
  h
} from "./chunk-6PAJ5Y37.js";

// frontend/comfyui/ui/output_canvas/presentation/vue/CanvasComparisonNavigation.ts
var CanvasComparisonNavigation = defineComponent({
  name: "CanvasComparisonNavigation",
  props: {
    snapshot: { type: Object, required: true },
    send: { type: Function, required: true },
    route: { type: Object, default: null },
    chooseRoute: { type: Function, required: true }
  },
  /** Bind both selectors to the same view authority used by browser and native overlays. */
  setup(props) {
    return () => h("div", { class: "sc-output-comparison-navigation" }, [
      h(
        "div",
        {
          class: "sc-output-tabstrip sc-output-compare-first",
          "data-sc-canvas-backdrop": "first",
          "data-sc-native-backdrop": props.snapshot.nativeBackdrop
        },
        [
          h(CanvasComparisonExit, { send: props.send }),
          props.route ? h(CanvasRoutePicker, { state: props.route, choose: props.chooseRoute }) : null,
          h(CanvasSelectionPicker, {
            sources: props.snapshot.sources,
            selection: props.snapshot.view.selection,
            label: "First output",
            send: props.send
          })
        ]
      ),
      h(
        "div",
        {
          class: "sc-output-tabstrip sc-output-compare-second",
          "data-sc-canvas-backdrop": "second",
          "data-sc-native-backdrop": props.snapshot.nativeBackdrop
        },
        [
          h(CanvasSelectionPicker, {
            sources: props.snapshot.sources,
            selection: props.snapshot.view.comparison?.secondary ?? null,
            label: "Comparison output",
            secondary: true,
            send: props.send
          })
        ]
      )
    ]);
  }
});

export {
  CanvasComparisonNavigation
};
