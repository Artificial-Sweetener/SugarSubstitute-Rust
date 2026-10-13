import {
  CanvasSelectionPicker
} from "./chunk-CNHJ3WOH.js";
import {
  CanvasBatchPicker
} from "./chunk-QLIZ5IFY.js";
import {
  defineComponent,
  h,
  onBeforeUnmount,
  onMounted,
  onUpdated,
  ref
} from "./chunk-LFPAN2S3.js";

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
    const strip = ref(null);
    const tabs = ref(null);
    const collapsed = ref(false);
    let observer = null;
    let observedTabs = null;
    function measure() {
      const parent = strip.value?.parentElement;
      if (!parent || !tabs.value || !parent.clientWidth) return;
      const fixed = [
        ...strip.value?.querySelectorAll("[data-sc-fixed-navigation]") ?? []
      ].reduce((width, element) => width + element.offsetWidth, 0);
      collapsed.value = tabs.value.scrollWidth + fixed + 24 > parent.clientWidth - 16;
    }
    onMounted(() => {
      if (typeof ResizeObserver === "undefined" || !strip.value?.parentElement) return;
      observer = new ResizeObserver(measure);
      observer.observe(strip.value.parentElement);
      measure();
    });
    onUpdated(() => {
      if (observedTabs !== tabs.value) {
        if (observedTabs) observer?.unobserve(observedTabs);
        observedTabs = tabs.value;
        if (observedTabs) observer?.observe(observedTabs);
      }
      measure();
    });
    onBeforeUnmount(() => observer?.disconnect());
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
      return h("div", { class: "sc-output-tabstrip", ref: strip }, [
        props.capabilities.commands.includes("batch") ? h("div", { "data-sc-fixed-navigation": "true" }, [
          h(CanvasBatchPicker, {
            count: Math.max(0, ...props.snapshot.sources.map((value) => value.batchCount)),
            grid: props.snapshot.view.mode === "overview",
            includeGrid: Boolean(source?.batchCount) && props.capabilities.commands.includes("overview"),
            selection,
            label: "Output image",
            send: props.send
          })
        ]) : null,
        props.snapshot.sources.length > 1 ? h(
          "nav",
          {
            class: collapsed.value ? "sc-output-tabs sc-output-tabs-measure" : "sc-output-tabs",
            ref: tabs,
            "aria-hidden": collapsed.value ? "true" : void 0,
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
                tabIndex: collapsed.value ? -1 : props.snapshot.view.selection?.sourceId === source2.id || index === 0 && !props.snapshot.sources.some(
                  (value) => value.id === props.snapshot.view.selection?.sourceId
                ) ? 0 : -1,
                title: [...source2.breadcrumb, source2.label].join(" / "),
                onClick: () => choose(index),
                onKeydown: (event) => navigate(event, index)
              },
              source2.label
            )
          )
        ) : null,
        collapsed.value ? h(CanvasSelectionPicker, {
          sources: props.snapshot.sources,
          selection,
          label: "Output source",
          preview: true,
          includeBatch: false,
          send: props.send
        }) : null
      ]);
    };
  }
});

export {
  CanvasSourceTabs
};
