import {
  ModelPickerWall
} from "./chunk-25UP3IXP.js";
import "./chunk-HP45EZMK.js";
import "./chunk-P7J5YKTZ.js";
import "./chunk-P6VGNXE5.js";
import {
  modelWallNeighbor
} from "./chunk-7ECUN2KM.js";
import {
  MODEL_DETECTOR_WALL_PROFILE
} from "./chunk-QBK6AEYB.js";
import "./chunk-5LUG4ZB5.js";
import "./chunk-K7ZHJNG4.js";
import "./chunk-SPVIVHMR.js";
import "./chunk-W2YTYCO4.js";
import "./chunk-FGK24KN5.js";
import {
  createApp,
  h,
  ref
} from "./chunk-OJTDXCRK.js";

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
