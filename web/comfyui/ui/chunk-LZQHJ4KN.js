import {
  defineComponent,
  h,
  nextTick,
  onBeforeUnmount,
  ref
} from "./chunk-LFPAN2S3.js";

// frontend/comfyui/ui/output_canvas/presentation/vue/CanvasAnchoredPicker.ts
var CanvasAnchoredPicker = defineComponent({
  name: "CanvasAnchoredPicker",
  props: {
    label: { type: String, required: true },
    value: { type: String, required: true },
    text: { type: String, default: "" },
    items: { type: Array, required: true },
    centered: { type: Boolean, default: false },
    choose: { type: Function, required: true }
  },
  /** Keep popup work finite and scoped to the control's owning document. */
  setup(props) {
    const anchor = ref(null);
    const popup = ref(null);
    const opened = ref(false);
    const active = ref(0);
    const geometry = ref({ left: 0, top: 0, width: 0, height: 0 });
    let nativePopup = false;
    let listening = null;
    function close(restoreFocus = false) {
      listening?.removeEventListener("pointerdown", outside, true);
      listening?.defaultView?.removeEventListener("resize", resized);
      listening = null;
      if (nativePopup && popup.value) popup.value.hidePopover();
      nativePopup = false;
      opened.value = false;
      if (restoreFocus) anchor.value?.focus();
    }
    function outside(event) {
      const target = event.target;
      if (target instanceof Node && !popup.value?.contains(target) && !anchor.value?.contains(target))
        close();
    }
    function resized() {
      close();
    }
    function focusRow() {
      const row = popup.value?.querySelectorAll('[role="option"]')[active.value];
      row?.focus({ preventScroll: true });
      row?.scrollIntoView?.({ block: "nearest" });
    }
    async function open() {
      if (opened.value) {
        close(true);
        return;
      }
      const button = anchor.value;
      const viewport = button?.ownerDocument.defaultView;
      if (!button || !viewport || !props.items.length) return;
      const rect = button.getBoundingClientRect();
      const rowHeight = Math.max(28, rect.height);
      const stride = rowHeight + 2;
      active.value = Math.max(
        0,
        props.items.findIndex((item) => item.value === props.value)
      );
      const above = Math.max(0, Math.floor((rect.top - 8) / stride));
      const below = Math.max(0, Math.floor((viewport.innerHeight - rect.bottom - 8) / stride));
      const visible = Math.min(props.items.length, above + 1 + below);
      const slot = Math.min(active.value, above, visible - 1);
      const width = Math.min(Math.max(rect.width, 34) + 16, viewport.innerWidth - 16);
      const height = visible * stride - 2 + 16;
      geometry.value = {
        left: Math.max(8, Math.min(rect.left - 8, viewport.innerWidth - width - 8)),
        top: Math.max(0, Math.min(rect.top - 8 - slot * stride, viewport.innerHeight - height)),
        width,
        height
      };
      opened.value = true;
      listening = button.ownerDocument;
      listening.addEventListener("pointerdown", outside, true);
      viewport.addEventListener("resize", resized);
      await nextTick();
      if (!opened.value || !popup.value) return;
      if (typeof popup.value.showPopover === "function") {
        popup.value.showPopover();
        nativePopup = true;
      }
      popup.value.scrollTop = Math.max(0, (active.value - slot) * stride);
      focusRow();
    }
    function select(index) {
      const item = props.items[index];
      if (!item) return;
      close(true);
      props.choose(item.value);
    }
    function keydown(event) {
      if (event.key === "Enter" || event.key === " ") {
        event.preventDefault();
        event.stopPropagation();
        select(active.value);
        return;
      }
      if (event.key === "Escape" || event.key === "Tab") {
        if (event.key === "Escape") event.preventDefault();
        event.stopPropagation();
        close(true);
        return;
      }
      const index = event.key === "ArrowDown" ? Math.min(props.items.length - 1, active.value + 1) : event.key === "ArrowUp" ? Math.max(0, active.value - 1) : event.key === "Home" ? 0 : event.key === "End" ? props.items.length - 1 : null;
      if (index === null) return;
      event.preventDefault();
      event.stopPropagation();
      active.value = index;
      focusRow();
    }
    onBeforeUnmount(() => close());
    return () => h(
      "span",
      { class: "sc-output-choice", "data-centered": props.centered ? "true" : void 0 },
      [
        h(
          "button",
          {
            ref: anchor,
            type: "button",
            class: "sc-output-segment",
            "aria-label": props.label,
            "aria-haspopup": "listbox",
            "aria-expanded": opened.value,
            disabled: !props.items.length,
            onClick: open
          },
          props.text || props.items.find((item) => item.value === props.value)?.label || "Output"
        ),
        opened.value ? h(
          "div",
          {
            ref: popup,
            class: "sc-output-picker-popup",
            popover: typeof anchor.value?.showPopover === "function" ? "manual" : void 0,
            role: "listbox",
            "aria-label": `${props.label} choices`,
            "data-centered": props.centered ? "true" : void 0,
            style: {
              left: `${geometry.value.left}px`,
              top: `${geometry.value.top}px`,
              width: `${geometry.value.width}px`,
              maxHeight: `${geometry.value.height}px`
            },
            onKeydown: keydown
          },
          props.items.map(
            (item, index) => h(
              "button",
              {
                key: item.value,
                type: "button",
                role: "option",
                "aria-selected": index === active.value,
                tabIndex: index === active.value ? 0 : -1,
                onFocus: () => {
                  active.value = index;
                },
                onClick: () => select(index)
              },
              item.label
            )
          )
        ) : null
      ]
    );
  }
});

export {
  CanvasAnchoredPicker
};
