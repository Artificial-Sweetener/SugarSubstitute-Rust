import {
  ModelVersionFamily
} from "./chunk-QJ2ZXLP2.js";
import "./chunk-VHI72ZTE.js";
import "./chunk-Z4V2DUNV.js";
import "./chunk-ZVGMUCWU.js";
import "./chunk-XU3535ZK.js";
import "./chunk-SPVIVHMR.js";
import {
  createApp,
  h,
  shallowRef
} from "./chunk-OJTDXCRK.js";

// frontend/comfyui/ui/model_library/vue/ModelVersionVueRuntime.ts
function mountModelVersionView(element, item, initial, onSelect) {
  const state = shallowRef(initial);
  const app = createApp({
    render: () => h(ModelVersionFamily, { item, ...state.value, onSelect })
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
  mountModelVersionView
};
