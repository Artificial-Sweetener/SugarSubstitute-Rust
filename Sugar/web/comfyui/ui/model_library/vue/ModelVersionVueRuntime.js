import {
  ModelVersionFamily
} from "./chunk-YW6TDWK2.js";
import "./chunk-V6H6SZH6.js";
import "./chunk-VHI72ZTE.js";
import "./chunk-ZVGMUCWU.js";
import "./chunk-EPSDIXQ5.js";
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
