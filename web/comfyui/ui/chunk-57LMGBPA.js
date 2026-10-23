import {
  CUBE_STACK_ROW_HEIGHT
} from "./chunk-3BUER5QA.js";
import {
  onBeforeUnmount,
  ref
} from "./chunk-6PAJ5Y37.js";

// frontend/comfyui/ui/surface/stack/vue/CubeStackPointerDrag.ts
function useCubeStackPointerDrag(options) {
  const lifted = ref(null);
  const order = ref(null);
  let candidate = null;
  let suppressClick = false;
  let frame = 0;
  let lastX = 0;
  let lastY = 0;
  const clear = (cancel = true) => {
    const root = options.root.value;
    const previous = candidate;
    candidate = null;
    root?.ownerDocument.defaultView?.cancelAnimationFrame(frame);
    frame = 0;
    if (previous && root?.hasPointerCapture?.(previous.pointerId))
      root.releasePointerCapture(previous.pointerId);
    root?.ownerDocument.removeEventListener("keydown", escape, true);
    root?.ownerDocument.removeEventListener("pointermove", move, true);
    root?.ownerDocument.removeEventListener("pointerup", finish, true);
    root?.ownerDocument.defaultView?.removeEventListener("blur", cancelDrag);
    if (cancel && lifted.value) {
      options.announce("Move canceled.");
      options.preview(null);
    }
    lifted.value = null;
    order.value = null;
  };
  const cancelDrag = () => {
    if (candidate) {
      clear();
      suppressClick = false;
    }
  };
  const escape = (event) => {
    if (event.key !== "Escape") return;
    event.preventDefault();
    event.stopPropagation();
    clear();
  };
  const project = () => {
    const root = options.root.value;
    const active = candidate;
    const ghost = lifted.value;
    if (!root || !active || !ghost) return;
    const bounds = root.getBoundingClientRect();
    if (lastX < bounds.left - 24 || lastX > bounds.right + 24) return;
    const list = root.querySelector(".sc-cube-stack__rows");
    if (!list) return;
    const current = [...order.value ?? active.original];
    const eligible = options.rows().filter((row) => row.reorderable);
    const ids = new Set(eligible.map((row) => row.id));
    const remaining = current.filter((id) => ids.has(id) && id !== active.id);
    const center = ghost.y + CUBE_STACK_ROW_HEIGHT / 2;
    const listTop = list.getBoundingClientRect().top;
    let index = remaining.length;
    for (const [slot, id] of remaining.entries()) {
      const destination = current.indexOf(id) * options.rowStride() + CUBE_STACK_ROW_HEIGHT / 2;
      if (center < listTop + destination) {
        index = slot;
        break;
      }
    }
    remaining.splice(index, 0, active.id);
    let cursor = 0;
    const projected = current.map((id) => ids.has(id) ? remaining[cursor++] ?? id : id);
    if (projected.every((id, slot) => id === current[slot])) return;
    order.value = projected;
    options.preview(projected);
  };
  const edgeScroll = () => {
    frame = 0;
    const root = options.root.value;
    const viewport = root?.parentElement;
    if (!root || !viewport || !lifted.value) return;
    const bounds = viewport.getBoundingClientRect();
    const delta = lastY < bounds.top + 32 ? -8 : lastY > bounds.bottom - 32 ? 8 : 0;
    if (delta && lastX >= bounds.left - 24 && lastX <= bounds.right + 24) {
      viewport.scrollTop += delta;
      project();
    }
    frame = root.ownerDocument.defaultView?.requestAnimationFrame(edgeScroll) ?? 0;
  };
  const start = (event, row) => {
    const root = options.root.value;
    const target = event.target;
    if (!root || candidate || !row.reorderable && !options.pending() || event.button !== 0 || target instanceof Element && target.closest(".sc-cube-stack-row__remove"))
      return;
    const element = event.currentTarget;
    if (!(element instanceof HTMLElement)) return;
    const bounds = element.getBoundingClientRect();
    event.stopPropagation();
    candidate = {
      id: row.id,
      pointerId: event.pointerId,
      startX: event.clientX,
      startY: event.clientY,
      offsetX: event.clientX - bounds.left,
      offsetY: event.clientY - bounds.top,
      width: bounds.width,
      original: options.rows().map((item) => item.id)
    };
    suppressClick = false;
    lastX = event.clientX;
    lastY = event.clientY;
    root.ownerDocument.addEventListener("keydown", escape, true);
    root.ownerDocument.addEventListener("pointermove", move, true);
    root.ownerDocument.addEventListener("pointerup", finish, true);
    root.ownerDocument.defaultView?.addEventListener("blur", cancelDrag);
  };
  const resume = () => {
    const active = candidate;
    if (!active || options.pending()) return;
    if (!lifted.value && Math.hypot(lastX - active.startX, lastY - active.startY) < 6) return;
    const row = options.rows().find((item) => item.id === active.id);
    if (!row?.reorderable) {
      clear();
      return;
    }
    if (!lifted.value) {
      candidate = { ...active, original: options.rows().map((item) => item.id) };
      options.root.value?.setPointerCapture(active.pointerId);
    }
    lifted.value = {
      row,
      x: lastX - active.offsetX,
      y: lastY - active.offsetY,
      width: active.width,
      grabX: active.offsetX,
      grabY: active.offsetY
    };
    suppressClick = true;
    if (!order.value) {
      order.value = candidate?.original ?? active.original;
      options.announce(`Moving ${row.label}. Drop at any Cube position.`);
      frame = options.root.value?.ownerDocument.defaultView?.requestAnimationFrame(edgeScroll) ?? 0;
    }
    project();
  };
  const move = (event) => {
    if (!candidate || event.pointerId !== candidate.pointerId) return;
    event.stopPropagation();
    lastX = event.clientX;
    lastY = event.clientY;
    if (Math.hypot(lastX - candidate.startX, lastY - candidate.startY) >= 6) {
      event.preventDefault();
      suppressClick = true;
    }
    resume();
  };
  const finish = (event) => {
    const active = candidate;
    if (!active || event.pointerId !== active.pointerId) return;
    event.stopPropagation();
    const bounds = options.root.value?.getBoundingClientRect();
    const viewport = options.root.value?.parentElement?.getBoundingClientRect();
    const valid = bounds && viewport && event.clientX >= bounds.left - 24 && event.clientX <= bounds.right + 24 && event.clientY >= viewport.top && event.clientY <= viewport.bottom;
    const final = order.value;
    const row = lifted.value?.row;
    if (!valid || !final || !row || final.every((id, index) => id === active.original[index])) {
      clear();
      return;
    }
    clear(false);
    options.commit(active.id, final);
  };
  const beginPointerInteraction = () => {
    if (!candidate) suppressClick = false;
  };
  const click = (event) => {
    if (!suppressClick || event.detail === 0) return;
    event.preventDefault();
    event.stopPropagation();
    suppressClick = false;
  };
  onBeforeUnmount(() => clear());
  return {
    lifted,
    order,
    start,
    move,
    resume,
    finish,
    cancel: cancelDrag,
    clear,
    click,
    beginPointerInteraction
  };
}

export {
  useCubeStackPointerDrag
};
