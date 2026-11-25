import {
  useModelImage
} from "./chunk-SPVIVHMR.js";
import {
  defineComponent,
  h
} from "./chunk-OJTDXCRK.js";

// frontend/comfyui/ui/model_library/vue/ModelCatalogImage.ts
var ModelCatalogImage = defineComponent({
  name: "SugarCubeModelCatalogImage",
  inheritAttrs: false,
  props: {
    image: { type: Object, required: true },
    fallback: { type: Boolean, default: false }
  },
  setup(props, { attrs }) {
    const image = useModelImage(
      () => props.image.url,
      () => props.image
    );
    return () => {
      const { url, unavailable } = image();
      if (url) return h("img", { ...attrs, src: url, alt: "", draggable: false });
      return unavailable && props.fallback ? h("span", { class: "sc-model-no-image" }, "No image") : null;
    };
  }
});

export {
  ModelCatalogImage
};
