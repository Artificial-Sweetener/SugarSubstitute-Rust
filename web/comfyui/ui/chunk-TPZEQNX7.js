import {
  CANVAS_COVERAGE_OPERATIONS,
  COVERAGE_PIXELS_MAX,
  COVERAGE_PIXELS_MIN,
  coverageOperation,
  coveragePixels
} from "./chunk-EFQNN4SR.js";
import {
  CANVAS_COVERAGE_OPERATION_LABELS,
  CANVAS_EDIT_TEXT
} from "./chunk-2WJX4EBF.js";
import {
  defineComponent,
  h
} from "./chunk-6PAJ5Y37.js";

// frontend/comfyui/ui/canvas_editing/presentation/CanvasCoverageControls.ts
var CanvasCoverageControls = defineComponent({
  name: "CanvasCoverageControls",
  props: {
    value: { type: Object, required: true },
    disabled: { type: Boolean, default: false },
    change: {
      type: Function,
      required: true
    }
  },
  setup(props) {
    return () => h("div", { class: "sc-canvas-edit-coverage-controls" }, [
      h(
        "select",
        {
          "aria-label": CANVAS_EDIT_TEXT.operation,
          "data-canvas-coverage": "operation",
          value: props.value.operation,
          disabled: props.disabled,
          onChange: (event) => {
            if (!(event.target instanceof HTMLSelectElement)) return;
            const operation = coverageOperation(event.target.value);
            if (operation && operation !== props.value.operation)
              props.change({ ...props.value, operation });
          }
        },
        CANVAS_COVERAGE_OPERATIONS.map(
          (operation) => h(
            "option",
            { value: operation, selected: operation === props.value.operation },
            CANVAS_COVERAGE_OPERATION_LABELS[operation]
          )
        )
      ),
      h("span", CANVAS_EDIT_TEXT.by),
      h("input", {
        type: "number",
        inputmode: "numeric",
        "aria-label": CANVAS_EDIT_TEXT.pixels,
        "data-canvas-coverage": "pixels",
        min: COVERAGE_PIXELS_MIN,
        max: COVERAGE_PIXELS_MAX,
        step: 1,
        value: props.value.pixels,
        disabled: props.disabled,
        onInput: (event) => {
          if (!(event.target instanceof HTMLInputElement)) return;
          const pixels = coveragePixels(event.target.value, props.value.pixels);
          if (pixels !== props.value.pixels) props.change({ ...props.value, pixels });
        },
        onChange: (event) => {
          if (event.target instanceof HTMLInputElement)
            event.target.value = String(props.value.pixels);
        }
      }),
      h("span", CANVAS_EDIT_TEXT.pixels)
    ]);
  }
});

export {
  CanvasCoverageControls
};
