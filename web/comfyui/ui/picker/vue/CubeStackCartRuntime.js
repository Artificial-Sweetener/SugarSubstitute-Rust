import {
  CubeStackCart
} from "../../chunk-JGLS4ZWV.js";
import "../../chunk-X5GLQTCN.js";
import "../../chunk-I5NPOT4R.js";
import "../../chunk-KQXDA27A.js";
import {
  createApp,
  h,
  shallowRef
} from "../../chunk-MNPWJMUZ.js";

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
