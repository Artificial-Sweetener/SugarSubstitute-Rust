import {
  defineComponent,
  h
} from "./chunk-MNPWJMUZ.js";

// frontend/comfyui/ui/output_canvas/presentation/vue/CanvasSourceTabs.ts
var CanvasSourceTabs = defineComponent({
  name: "CanvasSourceTabs",
  props: {
    snapshot: { type: Object, required: true },
    send: { type: Function, required: true }
  },
  /** Bind source navigation while preserving the selected batch position. */
  setup(props) {
    return () => h(
      "nav",
      { class: "sc-output-tabs", "aria-label": "Workflow output sources" },
      props.snapshot.sources.map(
        (source) => h(
          "button",
          {
            key: source.id,
            type: "button",
            "aria-pressed": props.snapshot.view.selection?.sourceId === source.id,
            title: [...source.breadcrumb, source.label].join(" / "),
            onClick: () => props.send({
              type: "select",
              selection: {
                sourceId: source.id,
                batchIndex: props.snapshot.view.selection?.batchIndex ?? 0
              }
            })
          },
          source.label
        )
      )
    );
  }
});

export {
  CanvasSourceTabs
};
