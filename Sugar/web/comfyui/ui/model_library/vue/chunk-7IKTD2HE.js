import {
  ModelUpdateButton
} from "./chunk-4DBFGV2O.js";
import {
  ModelGalleryTitle
} from "./chunk-HRRVXU3H.js";
import {
  MODEL_PICKER_WALL_PROFILE,
  layoutModelGallery,
  visibleModelRows
} from "./chunk-LCXHXGTS.js";
import {
  chooseModelThumbnail
} from "./chunk-EPSDIXQ5.js";
import {
  ModelCatalogImage
} from "./chunk-RPKXVHLS.js";
import {
  computed,
  defineComponent,
  h,
  nextTick,
  onBeforeUnmount,
  onMounted,
  ref,
  watch
} from "./chunk-3W5PGMRW.js";

// frontend/comfyui/ui/model_library/vue/ModelPickerWall.ts
var ModelPickerWall = defineComponent({
  name: "SugarCubesModelPickerWall",
  props: {
    items: { type: Array, required: true },
    selected: { type: String, required: true },
    active: { type: Number, required: true },
    optionPrefix: { type: String, required: true },
    updateOffers: { type: Array, default: () => [] },
    profile: {
      type: Object,
      default: () => MODEL_PICKER_WALL_PROFILE
    }
  },
  emits: {
    activate: (_value) => true,
    highlight: (_index) => true,
    context: (_item, _event) => true,
    geometry: (_width) => true,
    family: (_item) => true,
    updateContext: (_item, _event) => true
  },
  setup(props, { emit }) {
    const viewport = ref(null);
    const width = ref(540);
    const height = ref(580);
    const scrollTop = ref(0);
    const rows = computed(() => layoutModelGallery(props.items, width.value, props.profile));
    let observer = null;
    onMounted(() => {
      const element = viewport.value;
      if (!element) return;
      const measure = () => {
        if (element.clientWidth > 0) {
          width.value = element.clientWidth;
          emit("geometry", width.value);
        }
        if (element.clientHeight > 0) height.value = element.clientHeight;
      };
      measure();
      observer = new ResizeObserver(measure);
      observer.observe(element);
    });
    onBeforeUnmount(() => observer?.disconnect());
    watch(
      () => [props.active, props.items],
      () => {
        void nextTick(() => {
          const element = viewport.value;
          const row = rows.value.find(
            (candidate) => candidate.tiles.some((tile) => tile.index === props.active)
          );
          if (!element || !row) return;
          if (row.top < element.scrollTop) element.scrollTop = row.top;
          else if (row.top + row.height > element.scrollTop + height.value)
            element.scrollTop = Math.max(0, row.top + row.height - height.value);
          scrollTop.value = element.scrollTop;
        });
      },
      { immediate: true }
    );
    return () => h(
      "div",
      {
        ref: viewport,
        class: "sc-model-wall-viewport",
        "data-sc-scrollbar-edge": "parent",
        onScroll: (event) => {
          if (event.target instanceof HTMLElement) scrollTop.value = event.target.scrollTop;
        }
      },
      h(
        "div",
        {
          class: "sc-model-wall",
          role: "listbox",
          "aria-label": "Models",
          style: {
            height: `${String((rows.value.at(-1)?.top ?? 0) + (rows.value.at(-1)?.height ?? 0))}px`
          }
        },
        visibleModelRows(rows.value, scrollTop.value, height.value).flatMap(
          (row) => row.tiles.map((tile) => {
            const item = props.items[tile.index];
            if (!item) return null;
            const image = chooseModelThumbnail(
              item,
              "standard",
              Math.max(tile.width, tile.height) * devicePixelRatio
            );
            return h(
              "div",
              {
                key: item.id,
                id: `${props.optionPrefix}-option-${String(tile.index)}`,
                role: "option",
                tabindex: -1,
                class: [
                  "sc-model-tile",
                  {
                    "sc-model-current": item.value === props.selected,
                    "sc-model-active": tile.index === props.active
                  }
                ],
                "aria-selected": item.value === props.selected,
                "aria-label": [item.title, item.version].filter(Boolean).join(", "),
                title: item.relativePath,
                "data-model-value": item.value,
                style: {
                  left: `${String(tile.left)}px`,
                  top: `${String(tile.top)}px`,
                  width: `${String(tile.width)}px`,
                  height: `${String(tile.height)}px`
                },
                onPointerenter: () => emit("highlight", tile.index),
                onClick: () => emit("activate", item.value),
                onContextmenu: (event) => {
                  event.preventDefault();
                  event.stopPropagation();
                  emit("context", item, event);
                }
              },
              [
                image ? h(ModelCatalogImage, { image, fallback: true, lazy: true }) : h("span", { class: "sc-model-no-image" }, "No image"),
                h("span", { class: "sc-model-caption" }, [
                  h(ModelGalleryTitle, {
                    title: item.title,
                    active: tile.index === props.active
                  }),
                  item.version ? h("span", { class: "sc-model-version" }, item.version) : null
                ]),
                props.updateOffers.some((offer) => offer.modelId === item.id) ? h(ModelUpdateButton, {
                  onOpen: () => emit("family", item),
                  onContext: (event) => emit("updateContext", item, event)
                }) : null
              ]
            );
          })
        )
      )
    );
  }
});

export {
  ModelPickerWall
};
