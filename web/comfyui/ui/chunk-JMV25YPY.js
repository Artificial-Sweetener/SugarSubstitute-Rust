import {
  useCubeStackPointerDrag
} from "./chunk-P75ZYSJV.js";
import {
  CubeStackSelectionIndicator
} from "./chunk-DT7YXUO2.js";
import {
  CUBE_STACK_ROW_GAP,
  CUBE_STACK_ROW_STRIDE
} from "./chunk-GP5LDPUZ.js";
import {
  useCubeStackFocus
} from "./chunk-PB253IIL.js";
import {
  CubeStackRow
} from "./chunk-LDINNTDY.js";
import {
  computed,
  defineComponent,
  h,
  ref,
  watch
} from "./chunk-LFPAN2S3.js";

// frontend/comfyui/ui/surface/stack/vue/CubeStackRail.ts
var CubeStackRail = defineComponent({
  name: "SugarCubesStackRail",
  props: {
    interactive: { type: Boolean, default: true },
    state: { type: Object, required: true },
    actions: { type: Object, required: true }
  },
  setup(props) {
    const root = ref(null);
    const announcement = ref("");
    const awaiting = ref(false);
    const submittedOrder = ref(null);
    useCubeStackFocus(root);
    const gesture = useCubeStackPointerDrag({
      root,
      rows: () => props.state.rows,
      pending: () => props.state.pending || awaiting.value,
      preview: props.actions.previewOrder,
      announce: (message) => {
        announcement.value = message;
      },
      commit: (id, order) => {
        submittedOrder.value = order;
        void execute(() => props.actions.reorder(id, order), "Cube moved.");
      }
    });
    watch(
      () => props.interactive,
      (interactive) => {
        if (!interactive) gesture.clear();
      },
      { flush: "sync" }
    );
    const rows = computed(() => {
      const order = gesture.order.value ?? submittedOrder.value;
      if (!order) return props.state.rows;
      return [...props.state.rows].sort(
        (left, right) => order.indexOf(left.id) - order.indexOf(right.id)
      );
    });
    const clearPreview = () => {
      submittedOrder.value = null;
      props.actions.previewOrder(null);
    };
    watch(
      () => props.state.rows.map((row) => row.id).join("|"),
      () => {
        if (!awaiting.value && gesture.lifted.value) gesture.clear();
      }
    );
    watch(
      () => props.state.pending || awaiting.value,
      (pending) => {
        if (!pending) gesture.resume();
      }
    );
    const execute = async (work, label) => {
      awaiting.value = true;
      try {
        await work();
        announcement.value = label;
      } catch (error) {
        props.actions.reportError(error);
      } finally {
        awaiting.value = false;
        clearPreview();
      }
    };
    const keyboard = (event, row) => {
      if (!props.interactive) return;
      if (event.key !== "ArrowUp" && event.key !== "ArrowDown") return;
      const delta = event.key === "ArrowUp" ? -1 : 1;
      event.preventDefault();
      if (event.altKey) {
        const group = props.state.rows;
        if (!props.state.pending && !awaiting.value && row.reorderable && group[group.indexOf(row) + delta])
          void execute(
            () => props.actions.move(row.id, delta),
            `${row.label} moved ${delta < 0 ? "up" : "down"}.`
          );
        return;
      }
      const buttons = root.value?.querySelectorAll(
        ".sc-cube-stack-row__activate"
      );
      const index = rows.value.indexOf(row) + delta;
      buttons?.[index]?.focus({ preventScroll: false });
    };
    const tool = (icon, label, action) => h(
      props.state.appearance === "fluent" ? "fluent-button" : "button",
      {
        class: "sc-cube-stack__tool",
        type: "button",
        appearance: "subtle",
        size: "small",
        "icon-only": "",
        title: label,
        "aria-label": label,
        onClick: () => {
          if (props.interactive) action();
        }
      },
      [h("i", { class: `pi ${icon}`, "aria-hidden": "true" })]
    );
    return () => h(
      "nav",
      {
        ref: root,
        class: "sc-cube-stack",
        "aria-label": "Cube stack",
        "aria-busy": props.state.pending || void 0,
        "data-appearance": props.state.appearance,
        "data-kind": "rail",
        "data-compact": props.state.compact || void 0,
        "data-capture-wheel": "true",
        "data-pointer-dragging": gesture.lifted.value ? "" : void 0,
        onPointerdownCapture: gesture.beginPointerInteraction,
        onPointermove: gesture.move,
        onPointerup: gesture.finish,
        onPointercancel: gesture.cancel,
        onLostpointercapture: gesture.cancel,
        onClickCapture: gesture.click,
        onDragstartCapture: (event) => {
          event.preventDefault();
          event.stopPropagation();
        }
      },
      [
        h("div", { class: "sc-cube-stack__toolbar" }, [
          tool(
            props.state.compact ? "pi-angle-right" : "pi-angle-left",
            props.state.compact ? "Expand Cube stack" : "Compact Cube stack",
            () => props.actions.setCompact(!props.state.compact)
          ),
          tool("pi-times", "Hide Cube stack", props.actions.hide)
        ]),
        h(
          "ul",
          {
            class: "sc-cube-stack__rows",
            style: {
              height: `${String(Math.max(0, rows.value.length * CUBE_STACK_ROW_STRIDE - CUBE_STACK_ROW_GAP))}px`
            }
          },
          [
            h(CubeStackSelectionIndicator, {
              key: "selection",
              position: rows.value.some((row) => row.selected) ? rows.value.findIndex((row) => row.selected) : null
            }),
            ...rows.value.map(
              (row, index) => h(CubeStackRow, {
                key: row.id,
                row,
                style: { transform: `translateY(${String(index * CUBE_STACK_ROW_STRIDE)}px)` },
                compact: props.state.compact,
                pending: props.state.pending || !props.interactive,
                removable: !props.state.compact,
                draggable: false,
                "data-reorderable": row.reorderable && !props.state.pending && props.interactive || void 0,
                "data-dragging": gesture.lifted.value?.row.id === row.id || void 0,
                onPointerdown: (event) => {
                  if (props.interactive) gesture.start(event, row);
                },
                onKeydown: (event) => keyboard(event, row),
                activate: () => {
                  if (!gesture.lifted.value) props.actions.navigate(row.id);
                },
                remove: () => props.actions.remove(row.id),
                contextMenu: (event) => props.actions.contextMenu(row.id, event)
              })
            )
          ]
        ),
        gesture.lifted.value ? h(
          "div",
          {
            class: "sc-cube-stack__drag-overlay",
            "aria-hidden": "true",
            inert: true,
            style: {
              width: `${String(gesture.lifted.value.width)}px`,
              transform: `translate3d(${String(gesture.lifted.value.x)}px,${String(gesture.lifted.value.y)}px,0)`
            }
          },
          [
            h(CubeStackRow, {
              row: gesture.lifted.value.row,
              compact: props.state.compact,
              style: {
                transformOrigin: `${String(gesture.lifted.value.grabX)}px ${String(gesture.lifted.value.grabY)}px`
              },
              activate: () => void 0,
              remove: () => void 0,
              contextMenu: () => void 0
            })
          ]
        ) : null,
        h(
          props.state.appearance === "fluent" ? "fluent-button" : "button",
          {
            class: "sc-cube-stack__add",
            type: "button",
            appearance: "subtle",
            "aria-label": "Add cubes",
            title: "Add cubes",
            disabled: props.state.pending || !props.interactive,
            onClick: () => {
              if (props.interactive) props.actions.openCart();
            }
          },
          [h("i", { class: "pi pi-plus", "aria-hidden": "true" })]
        ),
        h(
          "span",
          { class: "sc-cube-stack__status", role: "status", "aria-live": "polite" },
          announcement.value
        )
      ]
    );
  }
});

export {
  CubeStackRail
};
