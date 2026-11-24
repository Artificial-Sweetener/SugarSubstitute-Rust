import {
  ModelChooser
} from "./chunk-TO2LJSGM.js";
import "./chunk-JK626FTQ.js";
import "./chunk-FK2KN4AM.js";
import "./chunk-FW25XVOZ.js";
import "./chunk-2WMWUCGK.js";
import "./chunk-CVOWWAQA.js";
import "./chunk-P7J5YKTZ.js";
import "./chunk-HP45EZMK.js";
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
  h
} from "./chunk-OJTDXCRK.js";

// frontend/comfyui/ui/model_library/vue/ModelChooserVueRuntime.ts
var mountIdentity = 0;
function openModelChooser(document, request, settled) {
  const container = document.createElement("div");
  document.body.append(container);
  let open = true;
  const settle = (item) => {
    if (!open) return;
    open = false;
    app.unmount();
    container.remove();
    settled(item);
  };
  const app = createApp({
    render: () => h(ModelChooser, {
      ...request,
      onChoose: (item) => settle(item),
      onDismiss: () => settle(null)
    })
  });
  app.config.idPrefix = `sc-model-chooser-${String(++mountIdentity)}`;
  app.mount(container);
  return {
    popup: container.querySelector(".sc-model-popup"),
    dismiss: () => settle(null)
  };
}
export {
  openModelChooser
};
