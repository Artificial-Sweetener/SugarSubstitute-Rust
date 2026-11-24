import {
  ModelChooser
} from "./chunk-ZTIBNUC5.js";
import "./chunk-JK626FTQ.js";
import "./chunk-IVMM67XL.js";
import "./chunk-BGLA25ZI.js";
import "./chunk-C7ZMR35V.js";
import "./chunk-7IKTD2HE.js";
import "./chunk-4DBFGV2O.js";
import "./chunk-HRRVXU3H.js";
import "./chunk-YSX5FCKB.js";
import "./chunk-RSAZHLEW.js";
import "./chunk-UM6L2UPZ.js";
import "./chunk-LCXHXGTS.js";
import "./chunk-EPSDIXQ5.js";
import "./chunk-RPKXVHLS.js";
import "./chunk-2G7SQCCL.js";
import "./chunk-NC5FFMF2.js";
import {
  createApp,
  h
} from "./chunk-3W5PGMRW.js";

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
