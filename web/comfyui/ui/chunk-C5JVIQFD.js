import {
  CanvasScenePicker
} from "./chunk-6MZ5BAVS.js";
import {
  CanvasSourceSlider
} from "./chunk-HFZ5NECD.js";
import {
  CanvasRoutePicker
} from "./chunk-P7TVKFNT.js";
import {
  CanvasSelectionPicker
} from "./chunk-KKVBN3OP.js";
import {
  CanvasBatchPicker
} from "./chunk-KQOAPFCP.js";
import {
  defineComponent,
  h,
  onBeforeUnmount,
  onMounted,
  onUpdated,
  ref
} from "./chunk-6PAJ5Y37.js";

// frontend/comfyui/ui/output_canvas/presentation/vue/CanvasNavigationBar.ts
var BACKDROP_BLUR_MARGIN = 24;
var CanvasNavigationBar = defineComponent({
  name: "CanvasNavigationBar",
  props: {
    snapshot: { type: Object, required: true },
    capabilities: { type: Object, required: true },
    send: { type: Function, required: true },
    route: { type: Object, default: null },
    chooseRoute: { type: Function, required: true },
    backdrop: { type: Object, default: null }
  },
  /** Bind source navigation while preserving the selected batch position. */
  setup(props) {
    const strip = ref(null);
    const slider = ref(null);
    const collapsed = ref(false);
    const backdrop = ref(null);
    let observer = null;
    let observedParent = null;
    let observedTabs = null;
    let observedStrip = null;
    let layerElement = null;
    let labels = null;
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
      observer ??= new ResizeObserver(() => {
        measure();
        locate();
      });
      if (strip.value !== observedStrip) {
        if (observedStrip) observer.unobserve(observedStrip);
        observedStrip = strip.value;
        if (observedStrip) observer.observe(observedStrip);
      }
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
    let origin = null;
    let placed = null;
    function locate() {
      const bar = strip.value;
      if (!bar) return;
      const bounds = bar.getBoundingClientRect();
      origin = { left: bounds.left + bar.clientLeft, top: bounds.top + bar.clientTop };
      placeBackdrop();
    }
    function placeBackdrop() {
      const layer = backdrop.value?.firstElementChild ?? null;
      const source = props.backdrop;
      if (layer !== layerElement) {
        layerElement = layer;
        placed = null;
      }
      if (!(layer instanceof HTMLElement) || !source) return;
      if (!origin) {
        locate();
        return;
      }
      const { left, top, width, height } = source.rect;
      const x = left - origin.left + BACKDROP_BLUR_MARGIN;
      const y = top - origin.top + BACKDROP_BLUR_MARGIN;
      const next = {
        url: source.url,
        size: `${String(width)}px ${String(height)}px`,
        position: `${String(x)}px ${String(y)}px`
      };
      if (placed?.url !== next.url)
        layer.style.backgroundImage = `url(${JSON.stringify(next.url)})`;
      if (placed?.size !== next.size) layer.style.backgroundSize = next.size;
      if (placed?.position !== next.position) layer.style.backgroundPosition = next.position;
      placed = next;
    }
    onMounted(() => {
      observe();
      labels = strip.value?.textContent ?? null;
      measure();
      locate();
    });
    onUpdated(() => {
      observe();
      const nextLabels = strip.value?.textContent ?? null;
      if (nextLabels !== labels) {
        labels = nextLabels;
        measure();
        locate();
      }
      placeBackdrop();
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
      const scenes = route?.route !== "input" && snapshot.scenes.length > 1;
      if (!route?.inputAvailable && !outputs && !scenes) return null;
      const selection = snapshot.view.selection;
      const source = snapshot.sources.find((value) => value.id === selection?.sourceId);
      return h("div", { class: "sc-output-tabstrip sc-output-navigation", ref: strip }, [
        props.backdrop ? h("span", { class: "sc-output-backdrop", ref: backdrop, "aria-hidden": "true" }, [
          h("span", { class: "sc-output-backdrop-image" })
        ]) : null,
        route ? h(CanvasRoutePicker, { state: route, choose: props.chooseRoute }) : null,
        scenes && capabilities.commands.includes("select-scene") ? h(CanvasScenePicker, {
          scenes: snapshot.scenes,
          scene: snapshot.view.scene,
          send: props.send
        }) : null,
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
