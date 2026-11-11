import {
  defineComponent,
  h
} from "./chunk-3W5PGMRW.js";

// frontend/comfyui/ui/model_library/vue/ModelUpdateButton.ts
var updateIcon = new URL("../../../../icons/model/update.svg", import.meta.url).href;
var ModelUpdateButton = defineComponent({
  name: "SugarCubesModelUpdateButton",
  emits: { open: () => true, context: (_event) => true },
  setup(_props, { emit }) {
    return () => h(
      "button",
      {
        type: "button",
        class: "sc-model-update",
        title: "Update available \u2014 view versions",
        "aria-label": "Update available \u2014 view versions",
        onPointerdown: (event) => event.stopPropagation(),
        onClick: (event) => {
          event.stopPropagation();
          emit("open");
        },
        onContextmenu: (event) => {
          event.preventDefault();
          event.stopPropagation();
          emit("context", event);
        }
      },
      h("span", {
        "aria-hidden": true,
        style: { "--sc-model-update-icon": `url("${updateIcon}")` }
      })
    );
  }
});

export {
  ModelUpdateButton
};
