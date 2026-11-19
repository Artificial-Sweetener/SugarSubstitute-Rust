// frontend/comfyui/ui/interaction/reorder/ReorderSlots.ts
function strideProjection(input) {
  const { current, key } = input;
  const movable = new Set(current.filter((item) => input.movable(item)));
  const remaining = current.filter((item) => movable.has(item) && item !== key);
  let index = remaining.length;
  for (const [slot, item] of remaining.entries()) {
    const destination = current.indexOf(item) * input.stride + input.size / 2;
    if (input.center < input.listStart + destination) {
      index = slot;
      break;
    }
  }
  remaining.splice(index, 0, key);
  let cursor = 0;
  const projected = current.map(
    (item) => movable.has(item) ? remaining[cursor++] ?? item : item
  );
  return projected.every((item, slot) => item === current[slot]) ? null : projected;
}

export {
  strideProjection
};
