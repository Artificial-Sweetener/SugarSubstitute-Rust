import {
  modelPickerPlacement
} from "./chunk-JK626FTQ.js";
import {
  ModelPickerPopupContent
} from "./chunk-IVMM67XL.js";
import {
  useModelBrowsing
} from "./chunk-YSX5FCKB.js";
import {
  defineComponent,
  h,
  nextTick,
  onBeforeUnmount,
  onMounted,
  ref,
  useId
} from "./chunk-3W5PGMRW.js";

// frontend/comfyui/ui/model_library/vue/ModelChooser.ts
var ModelChooser = defineComponent({
  name: "SugarCubesModelChooser",
  props: {
    items: { type: Array, required: true },
    /** Names the popup and its search box, e.g. "Search LoRA". */
    label: { type: String, required: true },
    /** The viewport point or rectangle the popup opens beside. */
    anchor: { type: Object, required: true }
  },
  emits: {
    choose: (_item) => true,
    dismiss: () => true
  },
  setup(props, { emit }) {
    const browsing = useModelBrowsing(() => props.items);
    const input = ref(null);
    const popup = ref(null);
    const popupId = useId();
    const geometry = modelPickerPlacement(
      { left: 0, top: 0, width: innerWidth, height: innerHeight },
      props.anchor
    );
    const placement = {
      left: `${String(geometry.left)}px`,
      top: `${String(geometry.top)}px`,
      width: `${String(geometry.width)}px`,
      height: `${String(geometry.height)}px`
    };
    const choose = (value) => {
      const item = browsing.visible.value.find((candidate) => candidate.value === value);
      if (item) emit("choose", item);
    };
    const outside = (event) => {
      if (event.target instanceof Node && !popup.value?.contains(event.target)) emit("dismiss");
    };
    const key = (event) => {
      event.stopPropagation();
      if (event.key === "Escape") {
        event.preventDefault();
        emit("dismiss");
      } else if (event.key === "Enter") {
        event.preventDefault();
        const item = browsing.visible.value[browsing.active.value];
        if (item) emit("choose", item);
      } else if (event.key === "Tab") {
        event.preventDefault();
        browsing.complete();
      } else if (browsing.move(event.key)) event.preventDefault();
    };
    onMounted(() => {
      document.addEventListener("pointerdown", outside, true);
      void nextTick(() => input.value?.focus());
    });
    onBeforeUnmount(() => document.removeEventListener("pointerdown", outside, true));
    return () => h(
      "div",
      {
        ref: popup,
        id: popupId,
        class: "sc-model-popup",
        role: "dialog",
        "aria-label": props.label,
        style: placement,
        onPointerdown: (event) => event.stopPropagation(),
        onWheel: (event) => event.stopPropagation()
      },
      [
        h("div", { class: "sc-model-field sc-model-field-open sc-model-popup-search" }, [
          h("input", {
            ref: input,
            value: browsing.query.value,
            placeholder: props.label,
            role: "combobox",
            "aria-label": props.label,
            "aria-expanded": true,
            "aria-controls": popupId,
            "aria-activedescendant": browsing.visible.value[browsing.active.value] ? `${popupId}-option-${String(browsing.active.value)}` : void 0,
            "aria-autocomplete": "both",
            spellcheck: false,
            onKeydown: key,
            onInput: (event) => {
              if (event.target instanceof HTMLInputElement) browsing.search(event.target.value);
            }
          }),
          browsing.query.value && browsing.completion.value ? h("span", { class: "sc-model-completion", "aria-hidden": true }, [
            h("span", { class: "sc-model-completion-prefix" }, browsing.query.value),
            browsing.completion.value.suffix
          ]) : null
        ]),
        h(ModelPickerPopupContent, {
          items: browsing.navigable.value,
          visible: browsing.visible.value,
          folder: browsing.folder.value,
          selected: "",
          active: browsing.active.value,
          optionPrefix: popupId,
          onNavigate: (path) => {
            browsing.open(path);
            input.value?.focus();
          },
          onSelect: choose,
          onGeometry: (width) => {
            browsing.wallWidth.value = width;
          }
        })
      ]
    );
  }
});

export {
  ModelChooser
};
