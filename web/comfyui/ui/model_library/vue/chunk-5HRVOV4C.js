import {
  defineComponent,
  h
} from "./chunk-57LGDI7T.js";

// frontend/comfyui/ui/model_library/vue/ModelProviderIcon.ts
var brands = {
  civitai: new URL("../../../../model-providers/civitai.svg", import.meta.url).href,
  openmodeldb: new URL("../../../../model-providers/openmodeldb.ico", import.meta.url).href
};
var globe = new URL("../../../../icons/model/globe.svg", import.meta.url).href;
function createModelProviderIcon(document, provider) {
  const icon = document.createElement("img");
  icon.className = "sc-model-provider-icon";
  icon.src = brands[provider] ?? globe;
  icon.alt = "";
  icon.draggable = false;
  return icon;
}
var ModelProviderIcon = defineComponent({
  name: "SugarCubesModelProviderIcon",
  props: { provider: { type: String, required: true } },
  setup(props) {
    return () => {
      const source = brands[props.provider];
      return source ? h("img", { class: "sc-model-provider-icon", src: source, alt: "", draggable: false }) : h("span", {
        class: "sc-model-provider-icon",
        "aria-hidden": "true",
        style: { "--sc-model-provider-icon": `url("${globe}")` }
      });
    };
  }
});

export {
  createModelProviderIcon,
  ModelProviderIcon
};
