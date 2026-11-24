import {
  ModelPickerField
} from "./chunk-N3A7DXHG.js";
import "./chunk-SNIFICEM.js";
import "./chunk-Z6JVJYNB.js";
import "./chunk-5NNW6U22.js";
import "./chunk-JK626FTQ.js";
import "./chunk-FK2KN4AM.js";
import "./chunk-FW25XVOZ.js";
import "./chunk-2WMWUCGK.js";
import "./chunk-CVOWWAQA.js";
import "./chunk-P7J5YKTZ.js";
import {
  shownModels
} from "./chunk-HP45EZMK.js";
import "./chunk-P6VGNXE5.js";
import "./chunk-5P2NA4VO.js";
import "./chunk-RSAZHLEW.js";
import "./chunk-UM6L2UPZ.js";
import "./chunk-LCXHXGTS.js";
import "./chunk-EPSDIXQ5.js";
import "./chunk-TOST2PZ5.js";
import "./chunk-CX6HVFIQ.js";
import "./chunk-W3WKORZN.js";
import {
  createApp,
  h,
  shallowRef
} from "./chunk-OJTDXCRK.js";

// frontend/comfyui/ui/model_library/vue/ModelPickerVueRuntime.ts
import { ModelDetectorDialog } from "../ModelDetectorDialog.js";
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
      detectorDialog: detectors ? (item) => new ModelDetectorDialog(document, item, detectors) : null,
      compatibility,
      session,
      discoveryAction: discoveryAction.value,
      onContextReady: (read) => {
        contextEntries = read;
      }
    })
  });
  const prioritize = actions?.prioritize?.bind(actions);
  if (prioritize)
    app.provide(shownModels, (shown) => {
      void prioritize(shown).catch(() => void 0);
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
