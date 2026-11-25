import {
  ModelChooser
} from "./chunk-LR3BJMGL.js";
import "./chunk-JK626FTQ.js";
import "./chunk-LENWKHDC.js";
import "./chunk-FW25XVOZ.js";
import "./chunk-2WMWUCGK.js";
import "./chunk-AHRH5QUN.js";
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
import "./chunk-25SOMJ7D.js";
import "./chunk-W2YTYCO4.js";
import "./chunk-SPVIVHMR.js";
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
  if (request.shown) app.provide(shownModels, request.shown);
  app.mount(container);
  return {
    popup: container.querySelector(".sc-model-popup"),
    dismiss: () => settle(null)
  };
}
export {
  openModelChooser
};
