import {
  CanvasSourceSlider
} from "./chunk-HFZ5NECD.js";
import {
  CanvasRoutePicker
} from "./chunk-CNPSIOKT.js";
import {
  CanvasSelectionPicker
} from "./chunk-OJSJCXDE.js";
import {
  CanvasBatchPicker
} from "./chunk-EPXRBNCY.js";
import {
  defineComponent,
  h,
  onBeforeUnmount,
  onMounted,
  onUpdated,
  ref
} from "./chunk-6PAJ5Y37.js";

// frontend/comfyui/ui/output_canvas/presentation/vue/CanvasNavigationBar.ts
var CanvasNavigationBar = defineComponent({
  name: "CanvasNavigationBar",
  props: {
    snapshot: { type: Object, required: true },
    capabilities: { type: Object, required: true },
    send: { type: Function, required: true },
    route: { type: Object, default: null },
    chooseRoute: { type: Function, required: true }
  },
  /** Bind source navigation while preserving the selected batch position. */
  setup(props) {
    const strip = ref(null);
    const slider = ref(null);
    const collapsed = ref(false);
    let observer = null;
    let observedParent = null;
    let observedTabs = null;
    function tabs() {
      const element = slider.value?.$el;
      return element instanceof HTMLElement ? element : null;
    }
    function measure() {
      const parent = strip.value?.parentElement;
      const list = tabs();
      if (!list) collapsed.value = false;
      if (!parent || !list || !parent.clientWidth) return;
      const fixed = [
        ...strip.value?.querySelectorAll("[data-sc-fixed-navigation]") ?? []
      ].reduce((width, element) => width + element.offsetWidth, 0);
      collapsed.value = list.scrollWidth + fixed + 24 > parent.clientWidth - 16;
    }
    function observe() {
      if (typeof ResizeObserver === "undefined") return;
      observer ??= new ResizeObserver(measure);
      const parent = strip.value?.parentElement ?? null;
      if (parent !== observedParent) {
        if (observedParent) observer.unobserve(observedParent);
        observedParent = parent;
        if (parent) observer.observe(parent);
      }
      const list = tabs();
      if (list !== observedTabs) {
        if (observedTabs) observer.unobserve(observedTabs);
        observedTabs = list;
        if (list) observer.observe(list);
      }
    }
    onMounted(() => {
      observe();
      measure();
    });
    onUpdated(() => {
      observe();
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
    return () => {
      const { snapshot, capabilities, route } = props;
      const outputs = route?.route !== "input" && capabilities.commands.includes("select-source") && (snapshot.sources.length > 1 || (snapshot.sources[0]?.batchCount ?? 0) > 1);
      if (!route?.inputAvailable && !outputs) return null;
      const selection = snapshot.view.selection;
      const source = snapshot.sources.find((value) => value.id === selection?.sourceId);
      return h("div", { class: "sc-output-tabstrip sc-output-navigation", ref: strip }, [
        route ? h(CanvasRoutePicker, { state: route, choose: props.chooseRoute }) : null,
        outputs && capabilities.commands.includes("batch") ? h("div", { "data-sc-fixed-navigation": "true" }, [
          h(CanvasBatchPicker, {
            count: Math.max(0, ...snapshot.sources.map((value) => value.batchCount)),
            grid: snapshot.view.mode === "overview",
            includeGrid: Boolean(source?.batchCount) && capabilities.commands.includes("overview"),
            selection,
            label: "Output image",
            send: props.send
          })
        ]) : null,
        outputs && snapshot.sources.length > 1 ? h(CanvasSourceSlider, {
          ref: slider,
          sources: snapshot.sources,
          selectedId: selection?.sourceId ?? null,
          measuring: collapsed.value,
          choose
        }) : null,
        outputs && collapsed.value ? h(CanvasSelectionPicker, {
          sources: snapshot.sources,
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
  CanvasNavigationBar
};
