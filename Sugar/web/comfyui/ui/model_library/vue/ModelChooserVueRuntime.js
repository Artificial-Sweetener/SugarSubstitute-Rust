import {
  ModelChooser
} from "./chunk-YUFXGF3L.js";
import "./chunk-JK626FTQ.js";
import "./chunk-VTTQW3W4.js";
import "./chunk-FW25XVOZ.js";
import "./chunk-2WMWUCGK.js";
import "./chunk-CX6HVFIQ.js";
import "./chunk-W3WKORZN.js";
import "./chunk-25UP3IXP.js";
import {
  shownModels
} from "./chunk-HP45EZMK.js";
import "./chunk-P7J5YKTZ.js";
import "./chunk-P6VGNXE5.js";
import "./chunk-TJCGDYFQ.js";
import "./chunk-RSAZHLEW.js";
import "./chunk-7ECUN2KM.js";
import "./chunk-QBK6AEYB.js";
import "./chunk-5LUG4ZB5.js";
import "./chunk-K7ZHJNG4.js";
import "./chunk-SPVIVHMR.js";
import "./chunk-W2YTYCO4.js";
import "./chunk-FGK24KN5.js";
import {
  createApp,
  h,
  shallowRef
} from "./chunk-OJTDXCRK.js";

// frontend/comfyui/ui/model_library/vue/ModelChooserVueRuntime.ts
var mountIdentity = 0;
function openModelChooser(document, request, settled) {
  const container = document.createElement("div");
  document.body.append(container);
  let open = true;
  const items = shallowRef(request.items.current());
  const unwatch = request.items.watch(() => {
    items.value = request.items.current();
  });
  const settle = (item) => {
    if (!open) return;
    open = false;
    unwatch();
    app.unmount();
    container.remove();
    settled(item);
  };
  const app = createApp({
    render: () => h(ModelChooser, {
      ...request,
      items: items.value,
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
