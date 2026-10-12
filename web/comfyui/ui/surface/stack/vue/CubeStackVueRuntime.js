import {
  CubeStackRail
} from "../../../chunk-JMV25YPY.js";
import "../../../chunk-P75ZYSJV.js";
import "../../../chunk-DT7YXUO2.js";
import "../../../chunk-GP5LDPUZ.js";
import "../../../chunk-PB253IIL.js";
import "../../../chunk-LDINNTDY.js";
import "../../../chunk-KMD23GAN.js";
import {
  createApp,
  h,
  shallowRef
} from "../../../chunk-LFPAN2S3.js";

// frontend/comfyui/ui/surface/stack/vue/CubeStackVueRuntime.ts
function mountCubeStackView(element, initial, actions) {
  const state = shallowRef(initial);
  const interactive = shallowRef(true);
  const app = createApp({
    render: () => h(CubeStackRail, { state: state.value, actions, interactive: interactive.value })
  });
  app.mount(element);
  return {
    update: (next) => {
      state.value = next;
    },
    setInteractive: (enabled) => {
      interactive.value = enabled;
    },
    dispose: () => app.unmount()
  };
}
export {
  mountCubeStackView
};
