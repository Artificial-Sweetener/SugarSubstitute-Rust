import {
  defineComponent,
  h,
  onBeforeUnmount,
  onMounted,
  onUpdated,
  ref
} from "./chunk-6PAJ5Y37.js";

// frontend/comfyui/ui/output_canvas/presentation/vue/CanvasSourceSlider.ts
var CanvasSourceSlider = defineComponent({
  name: "CanvasSourceSlider",
  props: {
    sources: { type: Array, required: true },
    selectedId: { type: String, default: null },
    measuring: { type: Boolean, default: false },
    choose: { type: Function, required: true }
  },
  /** Bind keyboard navigation and indicator placement to the rendered tabs. */
  setup(props) {
    const list = ref(null);
    const indicator = ref(null);
    let observer = null;
    function place() {
      const mark = indicator.value;
      const tab = list.value?.querySelector('[role="tab"][aria-selected="true"]');
      if (!mark) return;
      if (!tab) {
        mark.hidden = true;
        delete mark.dataset.placed;
        return;
      }
      mark.hidden = false;
      mark.style.transform = `translateX(${String(tab.offsetLeft)}px)`;
      mark.style.width = `${String(tab.offsetWidth)}px`;
      if (!mark.dataset.placed) {
        void mark.offsetWidth;
        mark.dataset.placed = "true";
      }
    }
    onMounted(() => {
      place();
      if (typeof ResizeObserver === "undefined" || !list.value) return;
      observer = new ResizeObserver(place);
      observer.observe(list.value);
    });
    onUpdated(place);
    onBeforeUnmount(() => observer?.disconnect());
    function navigate(event, index) {
      if (event.altKey || event.ctrlKey || event.metaKey || event.shiftKey) return;
      const count = props.sources.length;
      const next = event.key === "Home" ? 0 : event.key === "End" ? count - 1 : event.key === "ArrowRight" ? (index + 1) % count : event.key === "ArrowLeft" ? (index + count - 1) % count : null;
      if (next === null || !count) return;
      event.preventDefault();
      event.stopPropagation();
      list.value?.querySelectorAll('[role="tab"]')[next]?.focus();
      props.choose(next);
    }
    return () => {
      const known = props.sources.some((source) => source.id === props.selectedId);
      return h(
        "nav",
        {
          class: props.measuring ? "sc-output-tabs sc-output-tabs-measure" : "sc-output-tabs",
          ref: list,
          "aria-hidden": props.measuring ? "true" : void 0,
          role: "tablist",
          "aria-label": "Workflow output sources",
          "data-capture-wheel": "true"
        },
        [
          h("span", { class: "sc-output-slider-indicator", ref: indicator, "aria-hidden": "true" }),
          ...props.sources.map((source, index) => {
            const selected = source.id === props.selectedId;
            return h(
              "button",
              {
                key: source.id,
                type: "button",
                role: "tab",
                "aria-selected": selected,
                tabIndex: props.measuring ? -1 : selected || index === 0 && !known ? 0 : -1,
                title: [...source.breadcrumb, source.label].join(" / "),
                onClick: () => props.choose(index),
                onKeydown: (event) => navigate(event, index)
              },
              source.label
            );
          })
        ]
      );
    };
  }
});

export {
  CanvasSourceSlider
};
