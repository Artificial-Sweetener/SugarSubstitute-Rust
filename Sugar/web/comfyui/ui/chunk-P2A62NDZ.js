import {
  CANVAS_OPTION_SLIDERS
} from "./chunk-S7BMWHGE.js";
import {
  BRUSH_LIMITS,
  withBrushOption
} from "./chunk-EFQNN4SR.js";
import {
  CANVAS_EDIT_TEXT
} from "./chunk-OJDBLLHY.js";
import {
  canvasEditIcon
} from "./chunk-MOOZ7AAA.js";
import {
  defineComponent,
  h,
  inject,
  onBeforeUnmount,
  onMounted,
  ref,
  watch
} from "./chunk-ZMBAFDHL.js";

// frontend/comfyui/ui/canvas_editing/presentation/CanvasBrushOptions.ts
var gradients = 0;
var CanvasBrushOptions = defineComponent({
  name: "CanvasBrushOptions",
  props: {
    tool: { type: String, required: true },
    options: { type: Object, required: true },
    /** CSS colour of the active mask; the theme accent stands in when the host names none. */
    color: { type: String, default: null },
    change: {
      type: Function,
      required: true
    }
  },
  setup(props) {
    const expanded = ref(false);
    const draft = ref(null);
    const gradient = `sc-canvas-edit-tip-${++gradients}`;
    watch(
      () => props.tool,
      () => {
        draft.value = null;
      }
    );
    watch(
      () => props.options,
      (options) => {
        const pending = draft.value;
        if (pending && pending.size === options.size && pending.hardness === options.hardness && pending.opacity === options.opacity)
          draft.value = null;
      }
    );
    function update(field, value) {
      const next = withBrushOption(draft.value ?? props.options, field, value);
      draft.value = next;
      props.change(props.tool, next);
    }
    function slider(field, label, value) {
      return [
        h("label", { id: `${gradient}-${field}` }, label),
        h(BrushOptionSlider, {
          field,
          labelledBy: `${gradient}-${field}`,
          label,
          value: (draft.value ?? props.options)[field],
          change: (next) => update(field, next)
        }),
        h("span", { "data-canvas-option-value": field }, value)
      ];
    }
    return () => {
      const options = draft.value ?? props.options;
      const tint = props.tool === "eraser" ? "#ffffff" : props.color ?? "var(--p-primary-color,var(--ring,#df258e))";
      return h(
        "div",
        {
          class: "sc-canvas-edit-chrome sc-canvas-edit-options",
          "data-canvas-options": props.tool
        },
        [
          h("div", { class: "sc-canvas-edit-options-header" }, [
            h(
              "button",
              {
                type: "button",
                "aria-expanded": String(expanded.value),
                onClick: () => {
                  expanded.value = true;
                }
              },
              [tipPreview(gradient, options, tint), CANVAS_EDIT_TEXT.brushSettings]
            ),
            expanded.value ? h(
              "button",
              {
                type: "button",
                class: "sc-canvas-edit-icon-button",
                title: CANVAS_EDIT_TEXT.close,
                "aria-label": CANVAS_EDIT_TEXT.close,
                onClick: () => {
                  expanded.value = false;
                }
              },
              [canvasEditIcon("close")]
            ) : null
          ]),
          expanded.value ? h("div", { class: "sc-canvas-edit-sliders" }, [
            ...slider("size", CANVAS_EDIT_TEXT.size, CANVAS_EDIT_TEXT.pixelValue(options.size)),
            ...slider(
              "hardness",
              CANVAS_EDIT_TEXT.hardness,
              CANVAS_EDIT_TEXT.percentValue(options.hardness)
            ),
            ...slider(
              "opacity",
              CANVAS_EDIT_TEXT.opacity,
              CANVAS_EDIT_TEXT.percentValue(options.opacity)
            )
          ]) : null
        ]
      );
    };
  }
});
var BrushOptionSlider = defineComponent({
  name: "BrushOptionSlider",
  props: {
    field: { type: String, required: true },
    label: { type: String, required: true },
    labelledBy: { type: String, required: true },
    value: { type: Number, required: true },
    change: { type: Function, required: true }
  },
  setup(props) {
    const host = ref(null);
    const sliders = inject(CANVAS_OPTION_SLIDERS, null);
    let slider = null;
    const synchronize = () => {
      const limits = BRUSH_LIMITS[props.field];
      slider?.synchronize(limits.min, limits.max, 1, false, props.value);
    };
    onMounted(() => {
      const element = host.value;
      if (!element || !sliders)
        throw new Error("The canvas editing chrome was mounted without its option sliders.");
      slider = sliders(
        element.ownerDocument,
        {
          name: props.field,
          label: props.label,
          minimum: BRUSH_LIMITS[props.field].min,
          maximum: BRUSH_LIMITS[props.field].max,
          step: 1,
          precision: 0,
          attributes: { "data-canvas-option": props.field, "aria-labelledby": props.labelledBy }
        },
        (value) => {
          if (value !== props.value) props.change(value);
        }
      );
      element.append(slider.root);
      synchronize();
    });
    watch(() => props.value, synchronize);
    onBeforeUnmount(() => slider?.dispose());
    return () => h("span", { ref: host, class: "sc-canvas-edit-slider" });
  }
});
function tipPreview(id, options, color) {
  const hardness = `${options.hardness}%`;
  return h(
    "svg",
    {
      class: "sc-canvas-edit-tip",
      viewBox: "0 0 20 20",
      "aria-hidden": "true",
      "data-hardness": options.hardness,
      "data-opacity": options.opacity,
      "data-color": color
    },
    [
      h("defs", [
        h("radialGradient", { id }, [
          h("stop", { offset: "0%", style: { stopColor: color, stopOpacity: 1 } }),
          h("stop", { offset: hardness, style: { stopColor: color, stopOpacity: 1 } }),
          h("stop", { offset: "100%", style: { stopColor: color, stopOpacity: 0 } })
        ])
      ]),
      h("circle", {
        cx: 10,
        cy: 10,
        r: 8,
        fill: `url(#${id})`,
        "fill-opacity": options.opacity / 100
      })
    ]
  );
}

export {
  CanvasBrushOptions
};
