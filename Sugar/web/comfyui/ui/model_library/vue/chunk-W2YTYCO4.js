import {
  defineComponent,
  h,
  ref,
  watch
} from "./chunk-OJTDXCRK.js";

// frontend/comfyui/ui/model_library/vue/ModelImageElement.ts
var INCOMING_MODEL_IMAGE = "sc-model-image-incoming";
var ModelImageElement = defineComponent({
  name: "SugarCubesModelImageElement",
  inheritAttrs: false,
  props: {
    url: { type: String, required: true }
  },
  setup(props, { attrs }) {
    const front = ref(props.url);
    const incoming = ref(null);
    watch(
      () => props.url,
      (url) => {
        incoming.value = url === front.value ? null : url;
      }
    );
    const settle = (url, element) => {
      const ready = typeof element.decode === "function" ? element.decode() : Promise.resolve();
      void ready.then(
        () => {
          if (incoming.value !== url) return;
          front.value = url;
          incoming.value = null;
        },
        () => {
          if (incoming.value === url) incoming.value = null;
        }
      );
    };
    const image = (url, extra = {}) => h("img", { ...attrs, ...extra, key: url, src: url, alt: "", draggable: false });
    return () => {
      const next = incoming.value;
      if (next === null) return [image(front.value)];
      return [
        image(front.value),
        image(next, {
          class: [attrs.class, INCOMING_MODEL_IMAGE],
          onLoad: (event) => settle(next, event.currentTarget),
          onError: () => {
            if (incoming.value === next) incoming.value = null;
          }
        })
      ];
    };
  }
});

export {
  INCOMING_MODEL_IMAGE,
  ModelImageElement
};
