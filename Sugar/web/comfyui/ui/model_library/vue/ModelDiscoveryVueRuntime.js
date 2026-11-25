import {
  ModelDiscoveryGallery
} from "./chunk-Y7QSEX7O.js";
import "./chunk-VHI72ZTE.js";
import "./chunk-PTITF4WQ.js";
import "./chunk-NC2LRSUL.js";
import "./chunk-CON27HKR.js";
import "./chunk-ZVGMUCWU.js";
import "./chunk-W2YTYCO4.js";
import "./chunk-SPVIVHMR.js";
import "./chunk-CX6HVFIQ.js";
import "./chunk-W3WKORZN.js";
import {
  createApp,
  h,
  shallowRef
} from "./chunk-OJTDXCRK.js";

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
