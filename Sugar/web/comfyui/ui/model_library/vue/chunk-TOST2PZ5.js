import {
  defineComponent,
  h,
  ref,
  watch
} from "./chunk-OJTDXCRK.js";

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
    const shown = ref(null);
    const failed = ref(false);
    const attempt = ref(0);
    let loadedAttempt = 0;
    watch(
      () => props.image,
      (image, previous) => {
        if (image.url !== previous.url || failed.value) {
          attempt.value++;
          failed.value = false;
        }
      }
    );
    return () => {
      const wanted = props.image.url;
      const renderedAttempt = attempt.value;
      const settle = (loaded) => {
        if (renderedAttempt !== attempt.value || wanted !== props.image.url) return;
        if (!loaded) failed.value = true;
        else {
          if (shown.value === null) loadedAttempt = renderedAttempt;
          shown.value = wanted;
        }
      };
      if (shown.value === null && failed.value)
        return props.fallback ? h("span", { class: "sc-model-no-image" }, "No image") : null;
      const visible = h("img", {
        ...attrs,
        key: `visible:${String(shown.value === null ? renderedAttempt : loadedAttempt)}`,
        src: shown.value ?? wanted,
        alt: "",
        loading: props.lazy ? "lazy" : "eager",
        decoding: "async",
        draggable: false,
        ...shown.value === null ? { onLoad: () => settle(true), onError: () => settle(false) } : {}
      });
      return [
        visible,
        shown.value === null || shown.value === wanted || failed.value ? null : h("img", {
          key: `next:${wanted}:${String(renderedAttempt)}`,
          src: wanted,
          alt: "",
          "aria-hidden": "true",
          "data-sc-model-image-loading": "",
          decoding: "async",
          style: { display: "none" },
          onLoad: () => settle(true),
          onError: () => settle(false)
        })
      ];
    };
  }
});

export {
  ModelCatalogImage
};
