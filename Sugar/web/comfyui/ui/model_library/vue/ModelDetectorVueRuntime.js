import {
  ModelPickerWall
} from "./chunk-AHRH5QUN.js";
import "./chunk-P7J5YKTZ.js";
import "./chunk-HP45EZMK.js";
import "./chunk-P6VGNXE5.js";
import {
  modelWallNeighbor
} from "./chunk-XP42IRIX.js";
import {
  MODEL_DETECTOR_WALL_PROFILE
} from "./chunk-UAMK4WPC.js";
import "./chunk-XU3535ZK.js";
import "./chunk-25SOMJ7D.js";
import "./chunk-W2YTYCO4.js";
import "./chunk-SPVIVHMR.js";
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
