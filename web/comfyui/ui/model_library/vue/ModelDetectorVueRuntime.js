import {
  modelWallNeighbor
} from "./chunk-YR7F7J6A.js";
import {
  MODEL_DETECTOR_WALL_PROFILE,
  ModelPickerWall
} from "./chunk-JHL2BQUZ.js";
import "./chunk-JN3PQMAZ.js";
import "./chunk-HBDMWEN5.js";
import "./chunk-NU224JSW.js";
import "./chunk-TYXNDHG2.js";
import {
  createApp,
  h,
  ref
} from "./chunk-57LGDI7T.js";

// frontend/comfyui/ui/model_library/vue/ModelDetectorVueRuntime.ts
function mountDetectorWall(element, items, selected, onSelect) {
  const active = ref(
    Math.max(
      0,
      items.findIndex((item) => item.value === selected)
    )
  );
  const width = ref(540);
  const app = createApp({
    render: () => h(
      "div",
      {
        class: "sc-model-detector-wall",
        tabindex: 0,
        role: "group",
        "aria-label": "Detector thumbnails",
        onKeydown: (event) => {
          if (["ArrowLeft", "ArrowRight", "ArrowUp", "ArrowDown", "Home", "End"].includes(event.key)) {
            event.preventDefault();
            active.value = modelWallNeighbor(
              items,
              width.value,
              active.value,
              event.key,
              MODEL_DETECTOR_WALL_PROFILE
            );
          } else if (event.key === "Enter" || event.key === " ") {
            event.preventDefault();
            const item = items[active.value];
            if (item) onSelect(item.value);
          }
        }
      },
      h(ModelPickerWall, {
        items,
        selected,
        active: active.value,
        optionPrefix: "sc-detector",
        profile: MODEL_DETECTOR_WALL_PROFILE,
        onGeometry: (next) => {
          width.value = next;
        },
        onHighlight: (index) => {
          active.value = index;
        },
        onActivate: onSelect
      })
    )
  });
  app.mount(element);
  element.querySelector('[tabindex="0"]')?.focus();
  return () => app.unmount();
}
export {
  mountDetectorWall
};
