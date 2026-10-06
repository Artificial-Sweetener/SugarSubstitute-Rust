import {
  defineComponent,
  h,
  ref,
  watch
} from "./chunk-57LGDI7T.js";

// frontend/comfyui/ui/model_library/vue/ModelCatalogImage.ts
var ModelCatalogImage = defineComponent({
  name: "SugarCubeModelCatalogImage",
  inheritAttrs: false,
  props: {
    image: { type: Object, required: true },
    fallback: { type: Boolean, default: false },
    lazy: { type: Boolean, default: false }
  },
  setup(props, { attrs }) {
    const failed = ref(false);
    let attempt = 0;
    watch(
      () => props.image,
      (image, previous) => {
        if (image.url !== previous.url || failed.value) {
          attempt++;
          failed.value = false;
        }
      }
    );
    return () => {
      if (failed.value)
        return props.fallback ? h("span", { class: "sc-model-no-image" }, "No image") : null;
      const renderedAttempt = attempt;
      return h("img", {
        ...attrs,
        key: `${props.image.url}:${String(attempt)}`,
        src: props.image.url,
        alt: "",
        loading: props.lazy ? "lazy" : "eager",
        decoding: "async",
        draggable: false,
        onError: () => {
          if (renderedAttempt === attempt) failed.value = true;
        }
      });
    };
  }
});

export {
  ModelCatalogImage
};
