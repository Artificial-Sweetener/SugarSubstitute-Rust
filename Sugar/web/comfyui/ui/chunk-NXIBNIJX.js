import {
  CubeStackSelectionIndicator
} from "./chunk-OZITMUB2.js";
import {
  strideProjection
} from "./chunk-6DJIPVUI.js";
import {
  useReorderGesture
} from "./chunk-GB44H652.js";
import {
  useCubeStackFocus
} from "./chunk-RURRSIT4.js";
import {
  CubeStackRow
} from "./chunk-X3X3RHJX.js";
import {
  CUBE_STACK_ROW_HEIGHT,
  cubeStackRowGap,
  cubeStackRowStride
} from "./chunk-MGNQRLBB.js";
import {
  computed,
  defineComponent,
  h,
  ref,
  watch
} from "./chunk-ZMBAFDHL.js";

// frontend/comfyui/ui/surface/stack/vue/CubeStackRail.ts
import { buttonTag } from "./surface/appearance/ControlFactory.js";
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
    const rowStride = computed(() => cubeStackRowStride(props.state.packed));
    const busy = () => props.state.pending || awaiting.value;
    const REACH = 24;
    const beside = (x) => {
      const bounds = root.value?.getBoundingClientRect();
      return !!bounds && x >= bounds.left - REACH && x <= bounds.right + REACH;
    };
    const gesture = useReorderGesture({
      root,
      threshold: 6,
      // A press made while an earlier drop is still being applied waits for it.
      admission: (id) => busy() ? "defer" : props.state.rows.find((row) => row.id === id)?.reorderable ? "ready" : "refuse",
      keys: () => props.state.rows.map((row) => row.id),
      project: (card, current) => {
        const list = root.value?.querySelector(".sc-cube-stack__rows");
        if (!list) return null;
        const movable = new Set(
          props.state.rows.filter((row) => row.reorderable).map((row) => row.id)
        );
        return strideProjection({
          current,
          movable: (id) => movable.has(id),
          key: card.key,
          center: card.y + CUBE_STACK_ROW_HEIGHT / 2,
          listStart: list.getBoundingClientRect().top,
          stride: rowStride.value,
          size: CUBE_STACK_ROW_HEIGHT
        });
      },
      projects: (x) => beside(x),
      accepts: (x, y) => {
        const viewport = root.value?.parentElement?.getBoundingClientRect();
        return !!viewport && beside(x) && y >= viewport.top && y <= viewport.bottom;
      },
      scroll: {
        viewport: () => root.value?.parentElement ?? null,
        edge: 32,
        step: 8,
        lateral: REACH
      },
      lifted: (id) => {
        const label = props.state.rows.find((row) => row.id === id)?.label ?? "";
        announcement.value = `Moving ${label}. Drop at any Cube position.`;
      },
      preview: props.actions.previewOrder,
      commit: (id, order) => {
        submittedOrder.value = order;
        void execute(() => props.actions.reorder(id, order), "Cube moved.");
      },
      cancelled: (wasLifted) => {
        if (wasLifted) announcement.value = "Move canceled.";
      }
    });
    const liftedRow = computed(
      () => props.state.rows.find((row) => row.id === gesture.card.value?.key) ?? null
    );
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
        if (!awaiting.value && gesture.card.value) gesture.clear();
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
      buttonTag(props.state.appearance),
      {
        class: "sc-cube-stack__tool",
        type: "button",
        appearance: "subtle",
        "data-sc-button": "",
        quiet: "",
        size: props.state.appearance === "spectrum" ? "s" : "small",
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
        "data-pointer-dragging": gesture.card.value ? "" : void 0,
        onPointerdownCapture: gesture.beginInteraction,
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
              height: `${String(Math.max(0, rows.value.length * rowStride.value - cubeStackRowGap(props.state.packed)))}px`
            }
          },
          [
            h(CubeStackSelectionIndicator, {
              key: "selection",
              packed: props.state.packed,
              position: rows.value.some((row) => row.selected) ? rows.value.findIndex((row) => row.selected) : null
            }),
            ...rows.value.map(
              (row, index) => h(CubeStackRow, {
                key: row.id,
                row,
                style: { transform: `translateY(${String(index * rowStride.value)}px)` },
                compact: props.state.compact,
                pending: props.state.pending || !props.interactive,
                removable: !props.state.compact,
                draggable: false,
                "data-reorderable": row.reorderable && !props.state.pending && props.interactive || void 0,
                "data-dragging": gesture.card.value?.key === row.id || void 0,
                onPointerdown: (event) => {
                  const pressed = event.target;
                  if (props.interactive && !(pressed instanceof Element && pressed.closest(".sc-cube-stack-row__remove")))
                    gesture.press(event, row.id);
                },
                onKeydown: (event) => keyboard(event, row),
                activate: () => {
                  if (!gesture.card.value) props.actions.navigate(row.id);
                },
                remove: () => props.actions.remove(row.id),
                contextMenu: (event) => props.actions.contextMenu(row.id, event)
              })
            )
          ]
        ),
        gesture.card.value && liftedRow.value ? h(
          "div",
          {
            class: "sc-cube-stack__drag-overlay",
            "aria-hidden": "true",
            inert: true,
            style: {
              width: `${String(gesture.card.value.width)}px`,
              transform: `translate3d(${String(gesture.card.value.x)}px,${String(gesture.card.value.y)}px,0)`
            }
          },
          [
            h(CubeStackRow, {
              row: liftedRow.value,
              compact: props.state.compact,
              style: {
                transformOrigin: `${String(gesture.card.value.grabX)}px ${String(gesture.card.value.grabY)}px`
              },
              activate: () => void 0,
              remove: () => void 0,
              contextMenu: () => void 0
            })
          ]
        ) : null,
        h(
          buttonTag(props.state.appearance),
          {
            class: "sc-cube-stack__add",
            type: "button",
            appearance: "subtle",
            "data-sc-button": "",
            quiet: "",
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
