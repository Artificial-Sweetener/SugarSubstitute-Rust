import {
  defineComponent,
  h
} from "./chunk-MNPWJMUZ.js";

// frontend/comfyui/ui/output_canvas/presentation/vue/CanvasToolbar.ts
var CanvasToolbar = defineComponent({
  name: "CanvasToolbar",
  props: {
    snapshot: { type: Object, required: true },
    capabilities: { type: Object, required: true },
    send: { type: Function, required: true }
  },
  /** Bind inspection intents without introducing a presentation-owned camera. */
  setup(props) {
    function button(label, command, pressed, disabled = false) {
      if (!props.capabilities.commands.includes(command.type)) return null;
      return h(
        "button",
        {
          key: label,
          type: "button",
          disabled,
          ...pressed === void 0 ? {} : { "aria-pressed": pressed },
          onClick: () => props.send(command)
        },
        label
      );
    }
    return () => {
      const state = props.snapshot.view;
      const selection = state.selection;
      const selectedSource = props.snapshot.sources.find(
        (source) => source.id === selection?.sourceId
      );
      const count = selectedSource?.batchCount ?? 0;
      const batch = selection?.batchIndex ?? 0;
      return h(
        "div",
        { class: "sc-output-toolbar", role: "toolbar", "aria-label": "Output inspection" },
        [
          button(
            "Auto",
            { type: "follow", mode: state.follow === "auto" ? "manual" : "auto" },
            state.follow === "auto"
          ),
          button(
            "Overview",
            { type: "overview", enabled: state.mode !== "overview" },
            state.mode === "overview"
          ),
          button("Link", { type: "link", enabled: !state.linked }, state.linked),
          button("Fit", { type: "fit" }, void 0, state.mode === "overview"),
          button("1:1", { type: "native" }, void 0, state.mode === "overview"),
          button("Zoom out", { type: "zoom", factor: 0.8 }, void 0, state.mode === "overview"),
          button("Zoom in", { type: "zoom", factor: 1.25 }, void 0, state.mode === "overview"),
          button(
            "Previous image",
            { type: "batch", index: Math.max(0, batch - 1) },
            void 0,
            !selection || batch === 0
          ),
          h(
            "span",
            { class: "sc-output-batch", "aria-label": "Batch position" },
            props.snapshot.preview ? "Source preview" : selection ? state.comparison && batch >= count ? `Pinned image ${batch + 1}; latest batch ${count}` : `${batch + 1} / ${count}` : "No image"
          ),
          button(
            "Next image",
            { type: "batch", index: batch + 1 },
            void 0,
            !selection || batch + 1 >= count
          ),
          button(
            "Compare",
            { type: "comparison", enabled: !state.comparison },
            Boolean(state.comparison),
            state.mode === "overview" || !state.comparison && (!selection || !props.snapshot.selected || props.snapshot.preview || props.snapshot.resource.state !== "ready" || props.snapshot.selected.mediaKind !== "image")
          )
        ]
      );
    };
  }
});

export {
  CanvasToolbar
};
