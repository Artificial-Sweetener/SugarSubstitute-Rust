import {
  defineComponent,
  h
} from "./chunk-MNPWJMUZ.js";

// frontend/comfyui/ui/output_canvas/presentation/vue/CanvasCompareControls.ts
var CanvasCompareControls = defineComponent({
  name: "CanvasCompareControls",
  props: {
    snapshot: { type: Object, required: true },
    capabilities: { type: Object, required: true },
    send: { type: Function, required: true }
  },
  /** Wire secondary selection intents while retaining authority-owned comparison state. */
  setup(props) {
    return () => {
      const comparison = props.snapshot.view.comparison;
      if (!comparison) return null;
      const secondary = comparison.secondary;
      const source = props.snapshot.sources.find(
        (candidate) => candidate.id === secondary?.sourceId
      );
      return h("div", { class: "sc-output-comparison", "aria-label": "Comparison controls" }, [
        h("span", "B"),
        props.capabilities.commands.includes("select-comparison") ? h(
          "select",
          {
            "aria-label": "Comparison source",
            value: secondary?.sourceId ?? "",
            onChange: (event) => {
              const target = event.currentTarget;
              if (!(target instanceof HTMLSelectElement)) return;
              const chosen = props.snapshot.sources.find(
                (candidate) => candidate.id === target.value
              );
              if (chosen)
                props.send({
                  type: "select-comparison",
                  selection: { sourceId: chosen.id, batchIndex: secondary?.batchIndex ?? 0 }
                });
            }
          },
          [
            h("option", { value: "", disabled: true }, "Choose an output"),
            ...props.snapshot.sources.map(
              (candidate) => h("option", { key: candidate.id, value: candidate.id }, candidate.label)
            )
          ]
        ) : h("span", source?.label ?? "Unavailable output"),
        props.capabilities.commands.includes("select-comparison") ? h("input", {
          "aria-label": "Comparison batch",
          type: "number",
          min: 1,
          max: Math.max(1, source?.batchCount ?? 0),
          value: (secondary?.batchIndex ?? 0) + 1,
          disabled: !secondary,
          onChange: (event) => {
            const target = event.currentTarget;
            if (!(target instanceof HTMLInputElement)) return;
            const index = target.valueAsNumber - 1;
            if (secondary && Number.isSafeInteger(index) && index >= 0)
              props.send({
                type: "select-comparison",
                selection: { ...secondary, batchIndex: index }
              });
          }
        }) : null,
        props.capabilities.commands.includes("swap") ? h(
          "button",
          { type: "button", disabled: !secondary, onClick: () => props.send({ type: "swap" }) },
          "Swap A/B"
        ) : null,
        props.capabilities.commands.includes("wipe") ? h(
          "button",
          {
            type: "button",
            onClick: () => props.send({
              type: "wipe",
              split: comparison.split,
              orientation: comparison.orientation === "vertical" ? "horizontal" : "vertical"
            })
          },
          comparison.orientation === "vertical" ? "Horizontal split" : "Vertical split"
        ) : null
      ]);
    };
  }
});

export {
  CanvasCompareControls
};
