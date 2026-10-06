import {
  defineComponent,
  h,
  onBeforeUnmount,
  onMounted,
  onUpdated,
  ref
} from "./chunk-57LGDI7T.js";

// frontend/comfyui/ui/model_library/vue/ModelPickerRouteRow.ts
var ModelPickerRouteRow = defineComponent({
  name: "SugarCubesModelPickerRouteRow",
  props: { rowClass: { type: String, required: true } },
  setup(props, { slots }) {
    const element = ref(null);
    const overflowing = ref(false);
    let observer = null;
    const measure = () => {
      overflowing.value = !!element.value && element.value.scrollWidth > element.value.clientWidth;
    };
    onMounted(() => {
      observer = new ResizeObserver(measure);
      if (element.value) observer.observe(element.value);
      measure();
    });
    onUpdated(measure);
    onBeforeUnmount(() => observer?.disconnect());
    return () => h(
      "div",
      {
        ref: element,
        class: [props.rowClass, { "sc-model-route-overflow": overflowing.value }]
      },
      slots.default?.()
    );
  }
});

export {
  ModelPickerRouteRow
};
