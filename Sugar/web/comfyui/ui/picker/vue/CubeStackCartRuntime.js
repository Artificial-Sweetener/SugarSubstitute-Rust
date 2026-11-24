import {
  CubeStackCart
} from "../../chunk-ABVEWVJG.js";
import "../../chunk-6DJIPVUI.js";
import "../../chunk-GB44H652.js";
import "../../chunk-RURRSIT4.js";
import "../../chunk-X3X3RHJX.js";
import "../../chunk-P2EGKMGI.js";
import "../../chunk-MGNQRLBB.js";
import {
  createApp,
  h,
  shallowRef
} from "../../chunk-ZMBAFDHL.js";

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
