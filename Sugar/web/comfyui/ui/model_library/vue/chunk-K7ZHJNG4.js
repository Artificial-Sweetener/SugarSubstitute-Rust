import {
  useModelImage
} from "./chunk-SPVIVHMR.js";
import {
  ModelImageElement
} from "./chunk-W2YTYCO4.js";
import {
  ModelBlankCard
} from "./chunk-FGK24KN5.js";
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
      if (url) return h(ModelImageElement, { ...attrs, url });
      return unavailable && props.fallback ? h(ModelBlankCard, { state: "none" }) : null;
    };
  }
});

export {
  ModelCatalogImage
};
