import {
  CubeStackCart
} from "../../chunk-EM4F4IOV.js";
import "../../chunk-PB253IIL.js";
import "../../chunk-LDINNTDY.js";
import "../../chunk-KMD23GAN.js";
import {
  createApp,
  h,
  shallowRef
} from "../../chunk-LFPAN2S3.js";

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
