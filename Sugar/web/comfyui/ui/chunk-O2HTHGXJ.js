import {
  CubeStackIcon
} from "./chunk-VRNOEQN5.js";
import {
  defineComponent,
  h
} from "./chunk-6PAJ5Y37.js";

// frontend/comfyui/ui/surface/stack/vue/CubeStackRow.ts
var CubeStackRow = defineComponent({
  name: "SugarCubesStackRow",
  inheritAttrs: false,
  props: {
    row: { type: Object, required: true },
    compact: { type: Boolean, default: false },
    pending: { type: Boolean, default: false },
    removable: { type: Boolean, default: false },
    activate: { type: Function, required: true },
    remove: { type: Function, required: true },
    contextMenu: { type: Function, required: true }
  },
  setup(props, { attrs }) {
    return () => h(
      "li",
      {
        ...attrs,
        class: ["sc-cube-stack-row", attrs.class],
        "data-selected": props.row.selected || void 0,
        "data-bypassed": props.row.bypassed || void 0,
        "data-severity": props.row.severity,
        "data-row-id": props.row.id
      },
      [
        h(
          "button",
          {
            class: "sc-cube-stack-row__activate",
            type: "button",
            title: props.row.tooltip,
            "aria-label": props.row.label,
            "aria-description": props.row.targetModel ? `Model: ${props.row.targetModel}` : void 0,
            "aria-current": props.row.selected ? "true" : void 0,
            "aria-disabled": props.pending || void 0,
            onKeydown: (event) => {
              if (event.key !== "ContextMenu" && !(event.shiftKey && event.key === "F10")) return;
              event.preventDefault();
              const target = event.currentTarget;
              if (props.pending || !(target instanceof HTMLElement)) return;
              const bounds = target.getBoundingClientRect();
              props.contextMenu(
                new MouseEvent("contextmenu", { clientX: bounds.left, clientY: bounds.bottom })
              );
            },
            onClick: () => {
              if (!props.pending) props.activate();
            },
            onContextmenu: (event) => {
              event.preventDefault();
              if (!props.pending) props.contextMenu(event);
            }
          },
          [
            h(CubeStackIcon, { icon: props.row.icon, targetModel: props.row.targetModel }),
            h(
              "span",
              { class: "sc-cube-stack-row__text", "aria-hidden": props.compact || void 0 },
              [
                h("span", { class: "sc-cube-stack-row__label" }, props.row.label),
                h("span", { class: "sc-cube-stack-row__secondary" }, props.row.secondary)
              ]
            )
          ]
        ),
        props.removable ? h(
          "button",
          {
            class: "sc-cube-stack-row__remove",
            type: "button",
            title: `Remove ${props.row.label}`,
            "aria-label": `Remove ${props.row.label}`,
            "aria-disabled": props.pending || void 0,
            onClick: () => {
              if (!props.pending) props.remove();
            }
          },
          "\xD7"
        ) : null
      ]
    );
  }
});

export {
  CubeStackRow
};
