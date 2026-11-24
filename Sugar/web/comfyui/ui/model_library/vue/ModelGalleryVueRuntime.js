import {
  ModelPickerWall
} from "./chunk-CVOWWAQA.js";
import "./chunk-P7J5YKTZ.js";
import "./chunk-HP45EZMK.js";
import "./chunk-P6VGNXE5.js";
import {
  modelWallNeighbor
} from "./chunk-UM6L2UPZ.js";
import "./chunk-LCXHXGTS.js";
import "./chunk-EPSDIXQ5.js";
import "./chunk-TOST2PZ5.js";
import {
  createApp,
  h,
  shallowRef
} from "./chunk-OJTDXCRK.js";

// frontend/comfyui/ui/model_library/vue/ModelGalleryVueRuntime.ts
var mountIdentity = 0;
function mountModelGallery(element, events) {
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
