import {
  ModelDiscoveryGallery
} from "./chunk-7VOU5GOF.js";
import "./chunk-DOS5M4OO.js";
import "./chunk-MRDNZD2T.js";
import "./chunk-YSDVLBQQ.js";
import "./chunk-XR5PVBYE.js";
import "./chunk-3UMXK5CY.js";
import "./chunk-LTR7DIIA.js";
import "./chunk-5HRVOV4C.js";
import {
  createApp,
  h,
  shallowRef
} from "./chunk-57LGDI7T.js";

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
