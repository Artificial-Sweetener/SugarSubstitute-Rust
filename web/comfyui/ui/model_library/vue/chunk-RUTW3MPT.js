import {
  defineComponent,
  h
} from "./chunk-3W5PGMRW.js";

// frontend/comfyui/ui/model_library/vue/ModelTransferFill.ts
var ModelTransferFill = defineComponent({
  props: { progress: { type: Object, required: true } },
  setup(props) {
    return () => h("span", {
      class: [
        "sc-model-transfer-fill",
        { "sc-model-transfer-indeterminate": props.progress.percentage === null }
      ],
      role: "progressbar",
      "aria-label": "Model download",
      "aria-valuemin": 0,
      "aria-valuemax": 100,
      "aria-valuenow": props.progress.percentage ?? void 0,
      style: props.progress.percentage === null ? void 0 : { width: `${String(props.progress.percentage)}%` }
    });
  }
});

export {
  ModelTransferFill
};
