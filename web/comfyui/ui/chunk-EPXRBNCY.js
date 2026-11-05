import {
  CanvasAnchoredPicker
} from "./chunk-CIIISKSI.js";
import {
  defineComponent,
  h
} from "./chunk-6PAJ5Y37.js";

// frontend/comfyui/ui/output_canvas/presentation/vue/CanvasBatchPicker.ts
var CanvasBatchPicker = defineComponent({
  name: "CanvasBatchPicker",
  props: {
    count: { type: Number, required: true },
    grid: { type: Boolean, default: false },
    includeGrid: { type: Boolean, default: false },
    selection: { type: Object, default: null },
    label: { type: String, required: true },
    secondary: { type: Boolean, default: false },
    send: { type: Function, required: true }
  },
  /** Show batch positions and the source overview for the active workflow. */
  setup(props) {
    return () => {
      const count = props.count;
      const selection = props.selection;
      if (count <= 1 || !selection) return null;
      return h(CanvasAnchoredPicker, {
        label: props.label,
        value: props.grid ? "grid" : String(selection.batchIndex),
        text: props.grid ? "0" : String(selection.batchIndex + 1),
        centered: true,
        items: props.includeGrid ? [
          { value: "grid", label: "0" },
          ...Array.from({ length: count }, (_, index) => ({
            value: String(index),
            label: String(index + 1)
          }))
        ] : Array.from({ length: count }, (_, index) => ({
          value: String(count - index - 1),
          label: String(count - index)
        })),
        choose: (value) => {
          if (value === "grid" && props.includeGrid) {
            props.send({ type: "overview", enabled: true });
            return;
          }
          const index = Number(value);
          if (!Number.isSafeInteger(index) || index < 0 || index >= count) return;
          props.send(
            props.secondary ? { type: "select-comparison", selection: { ...selection, batchIndex: index } } : { type: "batch", index }
          );
        }
      });
    };
  }
});

export {
  CanvasBatchPicker
};
