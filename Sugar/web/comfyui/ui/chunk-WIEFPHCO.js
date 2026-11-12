import {
  defineComponent,
  h,
  onBeforeUnmount,
  shallowRef,
  watch
} from "./chunk-6PAJ5Y37.js";

// frontend/comfyui/ui/output_canvas/presentation/vue/CanvasWipeSeparator.ts
var CanvasWipeSeparator = defineComponent({
  name: "CanvasWipeSeparator",
  props: {
    snapshot: { type: Object, required: true },
    send: { type: Function, required: true }
  },
  /** Capture one divider pointer and coalesce normalized commands without image or camera access. */
  setup(props) {
    const local = shallowRef(null);
    let pointer = null;
    let pending = null;
    let cancel = null;
    function flush() {
      cancel?.();
      cancel = null;
      const split = pending;
      pending = null;
      const comparison = props.snapshot.view.comparison;
      if (split !== null && comparison)
        props.send({ type: "wipe", orientation: comparison.orientation, split });
    }
    function release(completed) {
      if (completed) flush();
      else {
        cancel?.();
        cancel = null;
        pending = null;
        local.value = null;
      }
      const captured = pointer;
      pointer = null;
      if (captured?.element.hasPointerCapture(captured.id))
        captured.element.releasePointerCapture(captured.id);
    }
    function move(event) {
      if (!pointer || pointer.id !== event.pointerId) return;
      const comparison = props.snapshot.view.comparison;
      const bounds = pointer.element.parentElement?.getBoundingClientRect();
      if (!comparison || !bounds) return;
      const vertical = comparison.orientation === "vertical";
      const size = vertical ? bounds.width : bounds.height;
      if (size <= 0) return;
      event.preventDefault();
      event.stopPropagation();
      const position = vertical ? event.clientX - bounds.left : event.clientY - bounds.top;
      const split = Math.max(0, Math.min(1, position / size));
      local.value = split;
      pending = split;
      const window = pointer.element.ownerDocument.defaultView;
      if (!cancel && window) {
        const frame = window.requestAnimationFrame(() => {
          cancel = null;
          flush();
        });
        cancel = () => window.cancelAnimationFrame(frame);
      }
    }
    function key(event) {
      const comparison = props.snapshot.view.comparison;
      if (!comparison || event.ctrlKey || event.metaKey || event.altKey || event.shiftKey) return;
      const negative = comparison.orientation === "vertical" ? "ArrowLeft" : "ArrowUp";
      const positive = comparison.orientation === "vertical" ? "ArrowRight" : "ArrowDown";
      const split = event.key === "Home" ? 0 : event.key === "End" ? 1 : event.key === negative ? Math.max(0, (local.value ?? comparison.split) - 0.05) : event.key === positive ? Math.min(1, (local.value ?? comparison.split) + 0.05) : null;
      if (split === null) return;
      event.preventDefault();
      event.stopPropagation();
      local.value = split;
      pending = split;
      flush();
    }
    watch(
      () => props.snapshot.revision,
      () => {
        if (!pointer) local.value = null;
      }
    );
    watch(
      () => props.snapshot.view.comparison?.orientation,
      () => release(false)
    );
    watch(
      () => JSON.stringify(props.snapshot.address),
      () => release(false)
    );
    onBeforeUnmount(() => release(false));
    return () => {
      const comparison = props.snapshot.view.comparison;
      if (!comparison) return null;
      const vertical = comparison.orientation === "vertical";
      const split = local.value ?? comparison.split;
      return h(
        "div",
        {
          class: "sc-output-wipe",
          role: "separator",
          tabindex: 0,
          "aria-label": "A/B comparison divider",
          "aria-orientation": comparison.orientation,
          "aria-valuemin": 0,
          "aria-valuemax": 100,
          "aria-valuenow": Math.round(split * 100),
          "data-orientation": comparison.orientation,
          style: vertical ? { left: `${split * 100}%` } : { top: `${split * 100}%` },
          onPointerdown: (event) => {
            const element = event.currentTarget;
            if (!(element instanceof HTMLElement) || pointer || event.button !== 0 || event.ctrlKey || event.metaKey || event.altKey || event.shiftKey)
              return;
            event.preventDefault();
            event.stopPropagation();
            pointer = { id: event.pointerId, element };
            element.focus({ preventScroll: true });
            element.setPointerCapture(event.pointerId);
          },
          onPointermove: move,
          onPointerup: (event) => {
            if (event.pointerId === pointer?.id) {
              event.stopPropagation();
              release(true);
            }
          },
          onPointercancel: (event) => {
            if (event.pointerId === pointer?.id) release(false);
          },
          onLostpointercapture: (event) => {
            if (event.pointerId === pointer?.id) release(false);
          },
          onKeydown: key
        },
        h("span", { "aria-hidden": "true" }, vertical ? "\u2194" : "\u2195")
      );
    };
  }
});

export {
  CanvasWipeSeparator
};
