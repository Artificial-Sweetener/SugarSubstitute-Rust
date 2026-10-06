import {
  ModelVersionFamily
} from "./chunk-TX757RKW.js";
import {
  createApp,
  h,
  shallowRef
} from "./chunk-57LGDI7T.js";

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
