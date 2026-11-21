import {
  CubeStackRail
} from "../../../chunk-HIIEY2OV.js";
import "../../../chunk-BSL67ZBW.js";
import "../../../chunk-6DJIPVUI.js";
import "../../../chunk-WUVKEBBQ.js";
import "../../../chunk-FMYVNRZD.js";
import "../../../chunk-5BPTM2TK.js";
import "../../../chunk-RGO7FEMV.js";
import "../../../chunk-MGNQRLBB.js";
import {
  createApp,
  h,
  shallowRef
} from "../../../chunk-6PAJ5Y37.js";

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
