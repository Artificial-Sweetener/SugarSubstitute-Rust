import {
  CubeStackCart
} from "../../chunk-6JB5WJVQ.js";
import "../../chunk-FMYVNRZD.js";
import "../../chunk-O2HTHGXJ.js";
import "../../chunk-VRNOEQN5.js";
import {
  createApp,
  h,
  shallowRef
} from "../../chunk-6PAJ5Y37.js";

// frontend/comfyui/ui/picker/vue/CubeStackCartRuntime.ts
function mountCubeStackCart(element, initial, actions) {
  const state = shallowRef(initial);
  const app = createApp({ render: () => h(CubeStackCart, { state: state.value, actions }) });
  app.mount(element);
  return {
    update: (next) => {
      state.value = next;
    },
    dispose: () => app.unmount(),
    focusSearch: () => element.querySelector(".sc-cube-stack-cart__search")?.focus()
  };
}
export {
  mountCubeStackCart
};
