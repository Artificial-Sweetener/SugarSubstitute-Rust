import {
  ModelPickerField
} from "./chunk-OW6YVQ3X.js";
import "./chunk-UHSVLRK7.js";
import "./chunk-VK2YTUDH.js";
import "./chunk-MMOUDFHF.js";
import "./chunk-OUW7MOZ3.js";
import "./chunk-TX757RKW.js";
import "./chunk-X3VJ7M63.js";
import "./chunk-3AFG44E5.js";
import "./chunk-SXT6EATL.js";
import "./chunk-WQY75U7Z.js";
import "./chunk-YR7F7J6A.js";
import "./chunk-JHL2BQUZ.js";
import "./chunk-JN3PQMAZ.js";
import "./chunk-HBDMWEN5.js";
import "./chunk-NU224JSW.js";
import "./chunk-TYXNDHG2.js";
import "./chunk-MRDNZD2T.js";
import "./chunk-YSDVLBQQ.js";
import "./chunk-3UMXK5CY.js";
import "./chunk-LTR7DIIA.js";
import "./chunk-5HRVOV4C.js";
import {
  createApp,
  h,
  shallowRef
} from "./chunk-57LGDI7T.js";

// frontend/comfyui/ui/model_library/vue/ModelPickerVueRuntime.ts
var mountIdentity = 0;
function mountModelPicker(element, binding, initialItems, actions = null, feedback = null, updates = null, detectors = null, outputs = null, compatibility = null, session = null) {
  const items = shallowRef(initialItems);
  const discoveryAction = shallowRef(null);
  let contextEntries = () => [];
  const app = createApp({
    render: () => h(ModelPickerField, {
      binding,
      items: items.value,
      actions,
      feedback,
      updates,
      detectors,
      outputs,
      compatibility,
      session,
      discoveryAction: discoveryAction.value,
      onContextReady: (read) => {
        contextEntries = read;
      }
    })
  });
  app.config.idPrefix = `sc-model-${String(++mountIdentity)}`;
  app.mount(element);
  return {
    contextEntries: () => contextEntries(),
    update: (value) => {
      items.value = value;
    },
    dispose: () => app.unmount(),
    updateDiscovery: (action) => {
      discoveryAction.value = action;
    }
  };
}
export {
  mountModelPicker
};
