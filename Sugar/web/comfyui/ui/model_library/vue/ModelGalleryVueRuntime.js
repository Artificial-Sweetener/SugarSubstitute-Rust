import {
  ModelPickerWall
} from "./chunk-25UP3IXP.js";
import {
  shownModels
} from "./chunk-HP45EZMK.js";
import "./chunk-P7J5YKTZ.js";
import "./chunk-P6VGNXE5.js";
import {
  modelWallNeighbor
} from "./chunk-7ECUN2KM.js";
import "./chunk-QBK6AEYB.js";
import "./chunk-5LUG4ZB5.js";
import "./chunk-K7ZHJNG4.js";
import "./chunk-SPVIVHMR.js";
import "./chunk-W2YTYCO4.js";
import "./chunk-FGK24KN5.js";
import {
  createApp,
  h,
  shallowRef
} from "./chunk-OJTDXCRK.js";

// frontend/comfyui/ui/model_library/vue/ModelGalleryVueRuntime.ts
var mountIdentity = 0;
function mountModelGallery(element, events, shown = null) {
  const items = shallowRef([]);
  const active = shallowRef(0);
  let width = 540;
  const identity = `sc-model-gallery-${String(++mountIdentity)}`;
  const app = createApp({
    render: () => h(ModelPickerWall, {
      items: items.value,
      selected: "",
      active: active.value,
      optionPrefix: identity,
      onActivate: (value) => {
        const index = items.value.findIndex((item) => item.value === value);
        if (index >= 0) events.activate(index);
      },
      onGeometry: (measured) => {
        width = measured;
      }
    })
  });
  app.config.idPrefix = identity;
  if (shown) app.provide(shownModels, shown);
  app.mount(element);
  return {
    update: (next, current) => {
      items.value = next;
      active.value = current;
    },
    neighbor: (current, key) => modelWallNeighbor(items.value, width, current, key),
    dispose: () => app.unmount()
  };
}
export {
  mountModelGallery
};
