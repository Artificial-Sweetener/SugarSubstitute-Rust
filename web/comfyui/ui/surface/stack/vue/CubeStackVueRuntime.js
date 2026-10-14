import {
  CubeStackRail
} from "../../../chunk-MWUR3A6U.js";
import "../../../chunk-LPTIR6EP.js";
import "../../../chunk-QPU7WOVZ.js";
import "../../../chunk-GP5LDPUZ.js";
import "../../../chunk-FMYVNRZD.js";
import "../../../chunk-O2HTHGXJ.js";
import "../../../chunk-VRNOEQN5.js";
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
