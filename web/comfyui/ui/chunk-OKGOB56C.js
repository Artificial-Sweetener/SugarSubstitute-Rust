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
    "Follow outputs",
    { type: "follow", mode: state.follow === "auto" ? "manual" : "auto" },
    false,
    state.follow === "auto"
  );
  action(
    "Overview",
    { type: "overview", enabled: state.mode !== "overview" },
    false,
    state.mode === "overview"
  );
  action("Linked pan and zoom", { type: "link", enabled: !state.linked }, false, state.linked);
  action("Fit", { type: "fit" }, state.mode === "overview");
  action("1:1", { type: "native" }, state.mode === "overview");
  action("Zoom out", { type: "zoom", factor: 0.8 }, state.mode === "overview");
  action("Zoom in", { type: "zoom", factor: 1.25 }, state.mode === "overview");
  action(
    "Compare",
    { type: "comparison", enabled: !comparison },
    state.mode === "overview" || !comparison && (!snapshot.selected || snapshot.preview || snapshot.resource.state !== "ready" || snapshot.selected.mediaKind !== "image"),
    Boolean(comparison)
  );
  if (comparison) {
    const secondary = comparison.secondary;
    if (capabilities.commands.includes("select-comparison")) {
      entries.push({
        label: "Compare with",
        children: snapshot.sources.map((source) => ({
          label: source.label,
          checked: secondary?.sourceId === source.id,
          select: () => send({
            type: "select-comparison",
            selection: { sourceId: source.id, batchIndex: secondary?.batchIndex ?? 0 }
          })
        }))
      });
      if (secondary) {
        const count = snapshot.sources.find((source) => source.id === secondary.sourceId)?.batchCount ?? 0;
        action(
          "Previous comparison image",
          {
            type: "select-comparison",
            selection: { ...secondary, batchIndex: Math.max(0, secondary.batchIndex - 1) }
          },
          secondary.batchIndex === 0
        );
        action(
          "Next comparison image",
          {
            type: "select-comparison",
            selection: { ...secondary, batchIndex: secondary.batchIndex + 1 }
          },
          secondary.batchIndex + 1 >= count
        );
      }
    }
    if (state.selection)
      action("Refresh first image", { type: "select", selection: state.selection });
    if (secondary)
      action("Refresh comparison image", { type: "select-comparison", selection: secondary });
    action("Swap images", { type: "swap" }, !secondary);
    action(comparison.orientation === "vertical" ? "Horizontal split" : "Vertical split", {
      type: "wipe",
      split: comparison.split,
      orientation: comparison.orientation === "vertical" ? "horizontal" : "vertical"
    });
  }
  const zoom = snapshot.zoom;
  const dimensions = zoom.dimensions;
  entries.push({
    label: `${zoom.mode === "fit" ? "Fit" : zoom.mode === "native" ? "1:1" : `${Math.round(zoom.physicalScale * 100)}%`}${dimensions ? ` \xB7 ${dimensions.width} \xD7 ${dimensions.height}` : ""}`,
    section: true
  });
  return entries;
}

export {
  canvasContextActions
};
