import {
  CubeStackRail
} from "../../../chunk-CGI3DVX4.js";
import "../../../chunk-3MJIV6IR.js";
import "../../../chunk-KLWNX2DM.js";
import "../../../chunk-GP5LDPUZ.js";
import "../../../chunk-X5GLQTCN.js";
import "../../../chunk-I5NPOT4R.js";
import "../../../chunk-KQXDA27A.js";
import {
  createApp,
  h,
  shallowRef
} from "../../../chunk-MNPWJMUZ.js";

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
