import {
  defineComponent,
  h
} from "./chunk-57LGDI7T.js";

// frontend/comfyui/ui/model_library/vue/ModelKeyRequiredBadge.ts
var keyIcon = new URL("../../../../icons/model/key.svg", import.meta.url).href;
var ModelKeyRequiredBadge = defineComponent({
  name: "SugarCubesModelKeyRequiredBadge",
  props: { providerName: { type: String, required: true } },
  setup(props) {
    return () => h("span", {
      class: "sc-model-key-required",
      title: `${props.providerName} API key required`,
      role: "img",
      "aria-label": `${props.providerName} API key required`
    });
  }
});
var modelKeyRequiredBadgeCss = `
.sc-model-key-required { position:absolute; width:24px; height:24px; border-radius:50%; border:1px solid #ffffff64; background:rgba(20,22,28,.88); pointer-events:none; }
.sc-model-key-required::after { content:''; position:absolute; inset:3px; background:#fff; mask:url("${keyIcon}") center/contain no-repeat; }
`;

export {
  ModelKeyRequiredBadge,
  modelKeyRequiredBadgeCss
};
