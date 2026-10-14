// frontend/comfyui/ui/output_canvas/domain/CanvasMedia.ts
function canvasMediaItem(snapshot, target) {
  const item = target === "primary" ? !snapshot.preview && snapshot.view.mode !== "overview" && snapshot.resource.state === "ready" ? snapshot.selected : null : target === "secondary" ? snapshot.secondaryResource?.state === "ready" ? snapshot.secondary : null : snapshot.grid.find((entry) => entry.source.id === target.sourceId)?.items.find((item2) => item2.id === target.itemId) ?? null;
  return item?.mediaKind === "image" ? item : null;
}

// frontend/comfyui/ui/output_canvas/presentation/vue/CanvasContextActions.ts
function canvasContextActions(snapshot, capabilities, send, target = "primary") {
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
  const available = canvasMediaItem(snapshot, target) !== null;
  if (snapshot.mediaActions)
    action(
      snapshot.mediaActions.fileAction === "reveal" ? "Reveal in folder" : "Download",
      { type: "file-media", target },
      !available
    );
  function copies(itemTarget, label, alternateLabel) {
    const disabled = canvasMediaItem(snapshot, itemTarget) === null;
    action(label, { type: "copy-media", target: itemTarget, format: "preferred" }, disabled);
    if (snapshot.mediaActions) {
      const format = snapshot.mediaActions.preferredCopyFormat === "jpeg" ? "png" : "jpeg";
      action(
        `${alternateLabel} .${format === "jpeg" ? "jpg" : "png"}`,
        { type: "copy-media", target: itemTarget, format },
        disabled
      );
    }
  }
  copies(target, "Copy", "Copy");
  if (comparison && target === "primary")
    copies("secondary", "Copy comparison image", "Copy comparison");
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
  canvasMediaItem,
  canvasContextActions
};
