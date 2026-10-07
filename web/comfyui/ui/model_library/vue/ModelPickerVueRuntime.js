import {
  ModelPickerField
} from "./chunk-ITPTXXE4.js";
import "./chunk-2EJWBG22.js";
import "./chunk-OKMKTXD6.js";
import "./chunk-RR5YCA3U.js";
import "./chunk-RUTW3MPT.js";
import "./chunk-CEVAK54V.js";
import "./chunk-EYWLUWSB.js";
import "./chunk-BGLA25ZI.js";
import "./chunk-FQAGC7FG.js";
import "./chunk-MM7W35BE.js";
import "./chunk-ZMF3A2RB.js";
import "./chunk-KJVBBJVU.js";
import "./chunk-4DBFGV2O.js";
import "./chunk-HBDMWEN5.js";
import "./chunk-HRRVXU3H.js";
import "./chunk-RPKXVHLS.js";
import "./chunk-6S3EI6RJ.js";
import "./chunk-KJ2VRCYH.js";
import "./chunk-LODHLXUH.js";
import "./chunk-SYMYA4CE.js";
import "./chunk-WIKCJTWX.js";
import "./chunk-NC5FFMF2.js";
import {
  createApp,
  h,
  shallowRef
} from "./chunk-3W5PGMRW.js";

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
