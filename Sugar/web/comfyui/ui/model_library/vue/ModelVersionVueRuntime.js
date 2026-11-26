import {
  ModelVersionFamily
} from "./chunk-PVPNOAZQ.js";
import "./chunk-VHI72ZTE.js";
import "./chunk-DYI5ZZC2.js";
import "./chunk-ZVGMUCWU.js";
import "./chunk-5LUG4ZB5.js";
import "./chunk-SPVIVHMR.js";
import "./chunk-W2YTYCO4.js";
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
