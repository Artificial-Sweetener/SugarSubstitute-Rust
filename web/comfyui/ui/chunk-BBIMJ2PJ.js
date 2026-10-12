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
    function choose(index) {
      const source = props.snapshot.sources[index];
      if (source)
        props.send({
          type: "select-source",
          selection: {
            sourceId: source.id,
            batchIndex: props.snapshot.view.selection?.batchIndex ?? 0
          }
        });
    }
    function navigate(event, index) {
      if (event.altKey || event.ctrlKey || event.metaKey || event.shiftKey) return;
      const count = props.snapshot.sources.length;
      const next = event.key === "Home" ? 0 : event.key === "End" ? count - 1 : event.key === "ArrowRight" ? (index + 1) % count : event.key === "ArrowLeft" ? (index + count - 1) % count : null;
      if (next === null || !count) return;
      event.preventDefault();
      event.stopPropagation();
      const button = event.currentTarget;
      if (button instanceof HTMLButtonElement)
        button.parentElement?.querySelectorAll('[role="tab"]')[next]?.focus();
      choose(next);
    }
    return () => h(
      "nav",
      {
        class: "sc-output-tabs",
        role: "tablist",
        "aria-label": "Workflow output sources",
        "data-capture-wheel": "true"
      },
      props.snapshot.sources.map(
        (source, index) => h(
          "button",
          {
            key: source.id,
            type: "button",
            role: "tab",
            "aria-selected": props.snapshot.view.selection?.sourceId === source.id,
            tabIndex: props.snapshot.view.selection?.sourceId === source.id || index === 0 && !props.snapshot.sources.some(
              (value) => value.id === props.snapshot.view.selection?.sourceId
            ) ? 0 : -1,
            title: [...source.breadcrumb, source.label].join(" / "),
            onClick: () => choose(index),
            onKeydown: (event) => navigate(event, index)
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
