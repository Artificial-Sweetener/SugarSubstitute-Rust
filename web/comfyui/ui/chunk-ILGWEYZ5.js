import {
  CanvasScenePicker
} from "./chunk-JDRPL54Q.js";
import {
  CanvasSourceSlider
} from "./chunk-HFZ5NECD.js";
import {
  CanvasRoutePicker
} from "./chunk-PJ5EUH5P.js";
import {
  CanvasSelectionPicker
} from "./chunk-EKZRFHOF.js";
import {
  CanvasBatchPicker
} from "./chunk-J75WOXTZ.js";
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
    function placeBackdrop() {
      const image = backdrop.value?.firstElementChild;
      const bar = strip.value;
      const source = props.backdrop;
      if (!(image instanceof HTMLElement) || !bar || !source) return;
      const bounds = bar.getBoundingClientRect();
      image.style.left = `${String(source.rect.left - bounds.left - bar.clientLeft)}px`;
      image.style.top = `${String(source.rect.top - bounds.top - bar.clientTop)}px`;
      image.style.width = `${String(source.rect.width)}px`;
      image.style.height = `${String(source.rect.height)}px`;
    }
    onMounted(() => {
      observe();
      measure();
      placeBackdrop();
    });
    onUpdated(() => {
      observe();
      measure();
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
          h("img", { src: props.backdrop.url, alt: "", draggable: false })
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
