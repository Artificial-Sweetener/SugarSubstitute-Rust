import {
  CANVAS_EDIT_TEXT,
  CANVAS_TOOL_LABELS
} from "./chunk-ECAVEMES.js";
import {
  CANVAS_TOOL_ICONS,
  canvasEditIcon,
  createCanvasEditIcon
} from "./chunk-X5ULNSRV.js";
import {
  defineComponent,
  h,
  onBeforeUnmount
} from "./chunk-6PAJ5Y37.js";

// frontend/comfyui/ui/canvas_editing/presentation/CanvasToolRail.ts
import { ContextMenu } from "./menus/ContextMenu.js";
var CanvasToolRail = defineComponent({
  name: "CanvasToolRail",
  props: {
    slots: { type: Array, required: true },
    selectSlot: { type: Function, required: true },
    selectTool: { type: Function, required: true }
  },
  setup(props) {
    let menu = null;
    onBeforeUnmount(() => menu?.close());
    function openFlyout(slot, trigger, event) {
      event.preventDefault();
      event.stopPropagation();
      const bounds = trigger.getBoundingClientRect();
      menu ??= new ContextMenu(trigger.ownerDocument);
      menu.open({
        label: CANVAS_EDIT_TEXT.toolGroup(CANVAS_TOOL_LABELS[slot.current.id]),
        anchor: event instanceof MouseEvent ? { kind: "pointer", x: event.clientX, y: event.clientY } : { kind: "submenu", bounds },
        trigger,
        attributes: { "data-canvas-edit-flyout": slot.id },
        entries: slot.members.map(
          (member) => ({
            label: CANVAS_TOOL_LABELS[member.id],
            selection: "single",
            checked: member.id === slot.current.id,
            disabled: !member.available,
            attributes: { "data-canvas-tool": member.id },
            icon: (document) => createCanvasEditIcon(document, CANVAS_TOOL_ICONS[member.id]),
            select: () => props.selectTool(member.id)
          })
        )
      });
    }
    return () => h(
      "div",
      {
        class: "sc-canvas-edit-chrome sc-canvas-edit-rail",
        role: "toolbar",
        "aria-orientation": "vertical",
        "aria-label": CANVAS_EDIT_TEXT.tools
      },
      props.slots.map(
        (slot) => h(
          "button",
          {
            key: slot.id,
            type: "button",
            "data-canvas-slot": slot.id,
            "data-canvas-tool": slot.current.id,
            title: CANVAS_TOOL_LABELS[slot.current.id],
            "aria-label": CANVAS_TOOL_LABELS[slot.current.id],
            "aria-pressed": String(slot.current.active),
            "aria-haspopup": slot.grouped ? "menu" : void 0,
            disabled: !slot.current.available,
            onClick: () => props.selectSlot(slot.id),
            onContextmenu: (event) => {
              if (slot.grouped && event.currentTarget instanceof HTMLElement)
                openFlyout(slot, event.currentTarget, event);
            },
            onKeydown: (event) => {
              const opens = event.key === "ContextMenu" || event.shiftKey && event.key === "F10";
              if (slot.grouped && opens && event.currentTarget instanceof HTMLElement)
                openFlyout(slot, event.currentTarget, event);
            }
          },
          [canvasEditIcon(CANVAS_TOOL_ICONS[slot.current.id])]
        )
      )
    );
  }
});

export {
  CanvasToolRail
};
