import {
  ModelUpdateButton
} from "./chunk-4DBFGV2O.js";
import {
  chooseModelThumbnail
} from "./chunk-HBDMWEN5.js";
import {
  ModelGalleryTitle
} from "./chunk-HRRVXU3H.js";
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

// frontend/comfyui/ui/model_library/ModelPickerLayout.ts
var MODEL_PICKER_WALL_PROFILE = {
  targetHeight: 198,
  minimumHeight: 156,
  maximumHeight: 258,
  minimumTileWidth: 126
};
var MODEL_DETECTOR_WALL_PROFILE = {
  targetHeight: 184,
  minimumHeight: 144,
  maximumHeight: 224,
  minimumTileWidth: 130
};
var MODEL_WALL_GUTTER = 4;
function aspect(value) {
  return Number.isFinite(value) && value > 0 ? Math.min(6, Math.max(0.2, value)) : 0.72;
}
function layoutModelWall(aspects, width, profile = MODEL_PICKER_WALL_PROFILE) {
  if (!aspects.length || !Number.isFinite(width) || width <= 0) return [];
  const values = aspects.map(aspect);
  const count = Math.max(
    1,
    Math.floor((width + MODEL_WALL_GUTTER) / (profile.minimumTileWidth + MODEL_WALL_GUTTER))
  );
  const candidates = solve(values, width, count, false, profile) ?? solve(values, width, count, true, profile) ?? [];
  let top = 0;
  return candidates.map((candidate) => {
    let left = 0;
    const row = {
      top,
      height: candidate.height,
      tiles: candidate.widths.map((tileWidth, offset) => {
        const tile = {
          index: candidate.start + offset,
          left,
          top,
          width: tileWidth,
          height: candidate.height
        };
        left += tileWidth + MODEL_WALL_GUTTER;
        return tile;
      })
    };
    top += candidate.height + MODEL_WALL_GUTTER;
    return row;
  });
}
function solve(values, width, count, relaxed, profile) {
  const costs = new Array(values.length + 1).fill(Infinity);
  const choices = new Array(values.length + 1).fill(null);
  costs[values.length] = 0;
  for (let start = values.length - 1; start >= 0; start--) {
    for (let end = start + 1; end <= Math.min(values.length, start + count); end++) {
      const ratios = values.slice(start, end);
      const available = width - (ratios.length - 1) * MODEL_WALL_GUTTER;
      const natural = available / Math.max(
        1e-3,
        ratios.reduce((sum, value) => sum + value, 0)
      );
      const final = end === values.length;
      const bounded = natural < profile.minimumHeight || natural > profile.maximumHeight;
      const candidates = [];
      if (!bounded || !final && relaxed) {
        const height = Math.min(profile.maximumHeight, Math.max(profile.minimumHeight, natural));
        const widths = ratios.map((ratio) => Math.max(1, ratio * height));
        const last = widths.length - 1;
        widths[last] = Math.max(
          1,
          (widths[last] ?? 1) + available - widths.reduce((sum, value) => sum + value, 0)
        );
        const coverage = (widths.reduce((sum, value) => sum + value, 0) + (widths.length - 1) * MODEL_WALL_GUTTER) / width;
        if (final || Math.abs(coverage - 1) <= 0.01) {
          candidates.push({
            start,
            end,
            height,
            widths,
            cost: final ? finalCost(height, coverage, ratios.length, width, false, profile) : Math.abs(height - profile.targetHeight) + Math.abs(height - natural) * 12 + (ratios.length === 1 ? 24 : 0)
          });
        }
      }
      if (final) {
        const height = Math.min(
          profile.maximumHeight,
          Math.max(1, Math.min(profile.targetHeight, natural))
        );
        const widths = ratios.map((ratio) => Math.max(1, ratio * height));
        const coverage = (widths.reduce((sum, value) => sum + value, 0) + (widths.length - 1) * MODEL_WALL_GUTTER) / width;
        candidates.push({
          start,
          end,
          height,
          widths,
          cost: finalCost(height, coverage, ratios.length, width, true, profile)
        });
      }
      for (const candidate of candidates) {
        const next = choices[end];
        const transition = next ? Math.abs(candidate.height - next.height) * (next.end === values.length ? 1.1 : 0.18) : 0;
        const cost = candidate.cost + transition + (costs[end] ?? Infinity);
        if (cost < (costs[start] ?? Infinity) - 1e-3) {
          costs[start] = cost;
          choices[start] = candidate;
        }
      }
    }
  }
  if (!Number.isFinite(costs[0])) return null;
  const rows = [];
  for (let index = 0; index < values.length; ) {
    const choice = choices[index];
    if (!choice) return null;
    rows.push(choice);
    index = choice.end;
  }
  return rows;
}
function finalCost(height, coverage, count, width, ragged, profile) {
  const comfort = Math.max(profile.minimumHeight * 0.9, profile.targetHeight * 0.82);
  return Math.abs(height - profile.targetHeight) + Math.max(0, (1 - coverage) * width) * 0.18 + Math.max(0, comfort - height) * 6.5 + Math.max(0, count - 7) * 65 + (ragged ? 18 : 0);
}
function visibleModelRows(rows, scrollTop, height) {
  const overscan = MODEL_PICKER_WALL_PROFILE.maximumHeight * 2;
  return rows.filter(
    (row) => row.top + row.height >= scrollTop - overscan && row.top <= scrollTop + height + overscan
  );
}

// frontend/comfyui/ui/model_library/ModelGalleryLayout.ts
function layoutModelGallery(items, width, profile = MODEL_PICKER_WALL_PROFILE) {
  return layoutModelWall(
    items.map((item) => {
      const image = chooseModelThumbnail(item, "standard", Number.POSITIVE_INFINITY);
      return image ? image.width / image.height : 0.72;
    }),
    width,
    profile
  );
}

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
  MODEL_PICKER_WALL_PROFILE,
  MODEL_DETECTOR_WALL_PROFILE,
  layoutModelGallery,
  ModelPickerWall
};
