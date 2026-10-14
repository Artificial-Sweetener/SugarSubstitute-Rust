import {
  ModelDiscoveryGallery
} from "./chunk-PREZUB3F.js";
import "./chunk-Y7OLUA3V.js";
import "./chunk-6S3EI6RJ.js";
import "./chunk-KJ2VRCYH.js";
import "./chunk-UBWFONUH.js";
import "./chunk-SYMYA4CE.js";
import "./chunk-WIKCJTWX.js";
import "./chunk-NC5FFMF2.js";
import {
  createApp,
  h,
  shallowRef
} from "./chunk-3W5PGMRW.js";

// frontend/comfyui/ui/model_library/vue/ModelDiscoveryVueRuntime.ts
function mountModelDiscoveryView(element, initial, actions) {
  const state = shallowRef(initial);
  const app = createApp({
    render: () => h(ModelDiscoveryGallery, { state: state.value, actions })
  });
  app.mount(element);
  return {
    update: (next) => {
      state.value = next;
    },
    dispose: () => app.unmount()
  };
}
export {
  mountModelDiscoveryView
};
