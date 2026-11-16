// frontend/comfyui/ui/canvas_editing/domain/CanvasEditTools.ts
var CANVAS_EDIT_SLOTS = Object.freeze([
  { id: "navigation", members: ["pan-zoom"] },
  { id: "move", members: ["move"] },
  { id: "brush", members: ["brush"] },
  { id: "eraser", members: ["eraser"] },
  { id: "selection", members: ["select-rectangle", "select-ellipse", "select-lasso"] },
  { id: "mask-shapes", members: ["mask-rectangle", "mask-ellipse", "mask-lasso"] },
  { id: "transform", members: ["transform", "shared-edges"] }
]);
function canvasEditSlotViews(state, remembered) {
  const offered = new Map(state.tools.map((tool) => [tool.id, tool.available]));
  const views = [];
  for (const slot of CANVAS_EDIT_SLOTS) {
    const members = slot.members.flatMap((id) => {
      const available = offered.get(id);
      return available === void 0 ? [] : [{ id, available, active: state.tool === id }];
    });
    const current = members.find((member) => member.active) ?? members.find((member) => member.id === remembered.get(slot.id)) ?? members[0];
    if (current) views.push({ id: slot.id, current, members, grouped: members.length > 1 });
  }
  return views;
}
function canvasEditSlotOf(tool) {
  return CANVAS_EDIT_SLOTS.find((slot) => slot.members.includes(tool))?.id ?? null;
}
function canvasEditToolAvailable(state, tool) {
  return state.tools.some((entry) => entry.id === tool && entry.available);
}

export {
  canvasEditSlotViews,
  canvasEditSlotOf,
  canvasEditToolAvailable
};
