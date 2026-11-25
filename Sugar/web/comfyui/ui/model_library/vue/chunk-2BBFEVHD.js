import {
  ModelTransferFill
} from "./chunk-73DOFFOF.js";
import {
  ModelImageElement
} from "./chunk-W2YTYCO4.js";
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
    return () => {
      const { url } = image();
      return h("div", { class: "sc-model-transfer-presentation" }, [
        url ? h(ModelImageElement, { class: "sc-model-transfer-thumbnail", url }) : null,
        h(ModelTransferFill, { progress: props.transfer }),
        h("span", { class: "sc-model-transfer-caption", "aria-hidden": "true" }, [
          h("span", null, "Downloading"),
          h("span", null, props.transfer.model.name)
        ])
      ]);
    };
  }
});

export {
  ModelTransferView
};
