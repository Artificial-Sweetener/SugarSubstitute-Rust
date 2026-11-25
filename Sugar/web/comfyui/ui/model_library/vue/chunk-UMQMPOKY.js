import {
  ModelTransferFill
} from "./chunk-73DOFFOF.js";
import {
  useModelImage
} from "./chunk-SPVIVHMR.js";
import {
  defineComponent,
  h
} from "./chunk-OJTDXCRK.js";

// frontend/comfyui/ui/model_library/vue/ModelTransferView.ts
var ModelTransferView = defineComponent({
  props: { transfer: { type: Object, required: true } },
  setup(props) {
    const image = useModelImage(() => props.transfer.model.previewUrl);
    return () => h("div", { class: "sc-model-transfer-presentation" }, [
      image().url ? h("img", {
        class: "sc-model-transfer-thumbnail",
        src: image().url,
        alt: "",
        draggable: false
      }) : null,
      h(ModelTransferFill, { progress: props.transfer }),
      h("span", { class: "sc-model-transfer-caption", "aria-hidden": "true" }, [
        h("span", null, "Downloading"),
        h("span", null, props.transfer.model.name)
      ])
    ]);
  }
});

export {
  ModelTransferView
};
