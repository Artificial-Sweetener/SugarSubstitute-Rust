import {
  ModelVersionFamily
} from "./chunk-DS56YYTF.js";
import "./chunk-6S3EI6RJ.js";
import "./chunk-KJ2VRCYH.js";
import "./chunk-SYMYA4CE.js";
import "./chunk-EPSDIXQ5.js";
import {
  createApp,
  h,
  shallowRef
} from "./chunk-3W5PGMRW.js";

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
