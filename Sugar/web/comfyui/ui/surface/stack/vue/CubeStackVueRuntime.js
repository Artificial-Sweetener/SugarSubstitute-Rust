import {
  CubeStackRail
} from "../../../chunk-NXIBNIJX.js";
import "../../../chunk-OZITMUB2.js";
import "../../../chunk-6DJIPVUI.js";
import "../../../chunk-GB44H652.js";
import "../../../chunk-RURRSIT4.js";
import "../../../chunk-X3X3RHJX.js";
import "../../../chunk-P2EGKMGI.js";
import "../../../chunk-MGNQRLBB.js";
import {
  createApp,
  h,
  shallowRef
} from "../../../chunk-ZMBAFDHL.js";

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
