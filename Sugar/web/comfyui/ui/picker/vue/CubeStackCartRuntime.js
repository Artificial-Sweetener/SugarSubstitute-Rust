import {
  CubeStackCart
} from "../../chunk-VCWOY2XU.js";
import "../../chunk-6DJIPVUI.js";
import "../../chunk-JDPP5GFL.js";
import "../../chunk-FMYVNRZD.js";
import "../../chunk-O2HTHGXJ.js";
import "../../chunk-VRNOEQN5.js";
import "../../chunk-MGNQRLBB.js";
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
