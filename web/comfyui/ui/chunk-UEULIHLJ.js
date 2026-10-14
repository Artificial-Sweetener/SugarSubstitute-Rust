import {
  CanvasBatchPicker
} from "./chunk-SVNSOY77.js";
import {
  CanvasAnchoredPicker
} from "./chunk-5NVJVFWB.js";
import {
  defineComponent,
  h
} from "./chunk-6PAJ5Y37.js";

// frontend/comfyui/ui/output_canvas/presentation/vue/CanvasSelectionPicker.ts
var CanvasSelectionPicker = defineComponent({
  name: "CanvasSelectionPicker",
  props: {
    sources: { type: Array, required: true },
    selection: { type: Object, default: null },
    label: { type: String, required: true },
    secondary: { type: Boolean, default: false },
    preview: { type: Boolean, default: false },
    includeBatch: { type: Boolean, default: true },
    send: { type: Function, required: true }
  },
  /** Send selections through the shared authority without choosing results in presentation code. */
  setup(props) {
    function chooseSource(value) {
      const source = props.sources.find((source2) => source2.id === value);
      if (!source) return;
      props.send({
        type: props.secondary ? "select-comparison" : props.preview ? "select-source" : "select",
        selection: { sourceId: source.id, batchIndex: props.selection?.batchIndex ?? 0 }
      });
    }
    return () => {
      return h("div", { class: "sc-output-selection-picker" }, [
        props.includeBatch ? h(CanvasBatchPicker, {
          count: Math.max(0, ...props.sources.map((value) => value.batchCount)),
          selection: props.selection,
          label: `${props.label} image`,
          secondary: props.secondary,
          send: props.send
        }) : null,
        h(CanvasAnchoredPicker, {
          label: props.label,
          value: props.selection?.sourceId ?? "",
          items: props.sources.map((source) => ({ value: source.id, label: source.label })),
          choose: chooseSource
        })
      ]);
    };
  }
});

export {
  CanvasSelectionPicker
};
