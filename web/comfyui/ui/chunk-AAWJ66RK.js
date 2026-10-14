// frontend/comfyui/ui/output_canvas/presentation/vue/CanvasContextActions.ts
function canvasContextActions(snapshot, capabilities, send) {
  const entries = [];
  const state = snapshot.view;
  const comparison = state.comparison;
  function action(label, command, disabled = false, checked) {
    if (!capabilities.commands.includes(command.type)) return;
    entries.push({
      label,
      disabled,
      ...checked === void 0 ? {} : { checked, selection: "multiple" },
      select: () => send(command)
    });
  }
  action(
    "Copy image",
    { type: "copy-media", target: "primary" },
    snapshot.preview || !snapshot.selected || snapshot.selected.mediaKind !== "image" || state.mode === "overview"
  );
  if (comparison)
    action(
      "Copy comparison image",
      { type: "copy-media", target: "secondary" },
      !snapshot.secondary || snapshot.secondary.mediaKind !== "image"
    );
  action(
    "Compare outputs",
    { type: "comparison", enabled: !comparison },
    state.mode === "overview" || !comparison && (!snapshot.comparisonAvailable || !snapshot.selected || snapshot.preview || snapshot.resource.state !== "ready" || snapshot.selected.mediaKind !== "image"),
    Boolean(comparison)
  );
  if (comparison) {
    const secondary = comparison.secondary;
    action("Swap images", { type: "swap" }, !secondary);
    action(comparison.orientation === "vertical" ? "Horizontal split" : "Vertical split", {
      type: "wipe",
      split: comparison.split,
      orientation: comparison.orientation === "vertical" ? "horizontal" : "vertical"
    });
  }
  return entries;
}

export {
  canvasContextActions
};
