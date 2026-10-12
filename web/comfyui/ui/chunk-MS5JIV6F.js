import {
  defineComponent,
  h
} from "./chunk-MNPWJMUZ.js";

// frontend/comfyui/ui/output_canvas/presentation/vue/CanvasSourceTabs.ts
var CanvasSourceTabs = defineComponent({
  name: "CanvasSourceTabs",
  props: {
    snapshot: { type: Object, required: true },
    capabilities: { type: Object, required: true },
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
    return () => {
      const selection = props.snapshot.view.selection;
      const source = props.snapshot.sources.find((value) => value.id === selection?.sourceId);
      const count = source?.batchCount ?? 0;
      const batch = selection?.batchIndex ?? 0;
      const navigation = [];
      if (count > 1 && selection && props.capabilities.commands.includes("batch"))
        navigation.push(
          h(
            "button",
            {
              type: "button",
              class: "sc-output-batch-arrow",
              "aria-label": "Previous image",
              disabled: batch === 0,
              onClick: () => props.send({ type: "batch", index: Math.max(0, batch - 1) })
            },
            "\u2039"
          ),
          h(
            "span",
            { class: "sc-output-batch", "aria-label": "Batch position" },
            props.snapshot.preview ? "Preview" : props.snapshot.view.comparison && batch >= count ? `Image ${batch + 1}` : `${batch + 1} / ${count}`
          ),
          h(
            "button",
            {
              type: "button",
              class: "sc-output-batch-arrow",
              "aria-label": "Next image",
              disabled: batch + 1 >= count,
              onClick: () => props.send({ type: "batch", index: batch + 1 })
            },
            "\u203A"
          )
        );
      return h("div", { class: "sc-output-tabstrip" }, [
        h("div", { class: "sc-output-batch-navigation" }, navigation),
        h(
          "nav",
          {
            class: "sc-output-tabs",
            role: "tablist",
            "aria-label": "Workflow output sources",
            "data-capture-wheel": "true"
          },
          props.snapshot.sources.map(
            (source2, index) => h(
              "button",
              {
                key: source2.id,
                type: "button",
                role: "tab",
                "aria-selected": props.snapshot.view.selection?.sourceId === source2.id,
                tabIndex: props.snapshot.view.selection?.sourceId === source2.id || index === 0 && !props.snapshot.sources.some(
                  (value) => value.id === props.snapshot.view.selection?.sourceId
                ) ? 0 : -1,
                title: [...source2.breadcrumb, source2.label].join(" / "),
                onClick: () => choose(index),
                onKeydown: (event) => navigate(event, index)
              },
              source2.label
            )
          )
        )
      ]);
    };
  }
});

export {
  CanvasSourceTabs
};
