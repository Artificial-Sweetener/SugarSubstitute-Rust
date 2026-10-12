import {
  CanvasSelectionPicker
} from "./chunk-YOT4H4QS.js";
import {
  defineComponent,
  h
} from "./chunk-LFPAN2S3.js";

// frontend/comfyui/ui/output_canvas/presentation/vue/CanvasComparisonNavigation.ts
var CanvasComparisonNavigation = defineComponent({
  name: "CanvasComparisonNavigation",
  props: {
    snapshot: { type: Object, required: true },
    send: { type: Function, required: true }
  },
  /** Bind both selectors to the same view authority used by browser and native overlays. */
  setup(props) {
    return () => h("div", { class: "sc-output-comparison-navigation" }, [
      h("div", { class: "sc-output-tabstrip sc-output-compare-first" }, [
        h(CanvasSelectionPicker, {
          sources: props.snapshot.sources,
          selection: props.snapshot.view.selection,
          label: "First output",
          send: props.send
        })
      ]),
      h("div", { class: "sc-output-tabstrip sc-output-compare-second" }, [
        h(CanvasSelectionPicker, {
          sources: props.snapshot.sources,
          selection: props.snapshot.view.comparison?.secondary ?? null,
          label: "Comparison output",
          secondary: true,
          send: props.send
        })
      ])
    ]);
  }
});

export {
  CanvasComparisonNavigation
};
