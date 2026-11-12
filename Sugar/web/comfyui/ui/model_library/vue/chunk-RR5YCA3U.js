import {
  ModelTransferFill
} from "./chunk-RUTW3MPT.js";
import {
  defineComponent,
  h,
  ref,
  watch
} from "./chunk-3W5PGMRW.js";

// frontend/comfyui/ui/model_library/vue/ModelTransferView.ts
var ModelTransferView = defineComponent({
  props: { transfer: { type: Object, required: true } },
  setup(props) {
    const failed = ref(false);
    watch(
      () => props.transfer.model.previewUrl,
      () => {
        failed.value = false;
      }
    );
    return () => h("div", { class: "sc-model-transfer-presentation" }, [
      props.transfer.model.previewUrl && !failed.value ? h("img", {
        class: "sc-model-transfer-thumbnail",
        src: props.transfer.model.previewUrl,
        alt: "",
        draggable: false,
        decoding: "async",
        onError: () => {
          failed.value = true;
        }
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
