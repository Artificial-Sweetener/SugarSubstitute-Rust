import {
  ModelPickerField
} from "./chunk-ITFEP25U.js";
import "./chunk-5NNW6U22.js";
import "./chunk-UMQMPOKY.js";
import "./chunk-73DOFFOF.js";
import "./chunk-JK626FTQ.js";
import "./chunk-OMRMSSWC.js";
import "./chunk-FW25XVOZ.js";
import "./chunk-2WMWUCGK.js";
import "./chunk-PYZ44I7C.js";
import "./chunk-P7J5YKTZ.js";
import {
  shownModels
} from "./chunk-HP45EZMK.js";
import "./chunk-P6VGNXE5.js";
import "./chunk-TGMBNLE7.js";
import "./chunk-RSAZHLEW.js";
import "./chunk-XP42IRIX.js";
import "./chunk-UAMK4WPC.js";
import "./chunk-XU3535ZK.js";
import "./chunk-OLJZ2VLG.js";
import "./chunk-SPVIVHMR.js";
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
