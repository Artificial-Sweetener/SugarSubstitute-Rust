// frontend/comfyui/ui/output_canvas/domain/CanvasMedia.ts
function canvasMediaItem(snapshot, target) {
  const item = canvasMediaFile(snapshot, target);
  return item?.mediaKind === "image" ? item : null;
}
function canvasMediaFile(snapshot, target) {
  const item = target === "primary" ? !snapshot.preview && snapshot.view.mode !== "overview" && snapshot.resource.state === "ready" ? snapshot.selected : null : target === "secondary" ? snapshot.secondaryResource?.state === "ready" ? snapshot.secondary : null : snapshot.grid.find((entry) => entry.source.id === target.sourceId)?.items.find((item2) => item2.id === target.itemId) ?? null;
  return item && item.mediaKind !== "unsupported" && !item.unavailable ? item : null;
}

// frontend/comfyui/ui/output_canvas/presentation/vue/CanvasContextActions.ts
function canvasContextActions(snapshot, capabilities, send, target = "primary") {
  const state = snapshot.view;
  const comparison = state.comparison;
  const tile = typeof target === "object";
  if (state.mode === "overview" && !tile && !comparison) return [];
  const entries = [];
  function action(label, command, disabled = false, checked) {
    if (!capabilities.commands.includes(command.type)) return;
    entries.push({
      label,
      disabled,
      ...checked === void 0 ? {} : { checked, selection: "multiple" },
      select: () => send(command)
    });
  }
  const video = targetItem(snapshot, target)?.mediaKind === "video";
  if (!video && (comparison || state.mode !== "overview" && snapshot.comparisonAvailable))
    action(
      "Compare outputs",
      { type: "comparison", enabled: !comparison },
      !comparison && (!snapshot.selected || snapshot.preview || snapshot.resource.state !== "ready" || snapshot.selected.mediaKind !== "image"),
      Boolean(comparison)
    );
  const comparisonCount = entries.length;
  const unavailable = canvasMediaItem(snapshot, target) === null;
  const item = targetItem(snapshot, target);
  const actions = item ? snapshot.itemActions?.find((actions2) => actions2.itemId === item.id) : null;
  action(
    "Copy",
    { type: "copy-media", target, format: "preferred" },
    unavailable || actions?.copy === false
  );
  if (!video && snapshot.mediaActions?.photoshop) {
    action(
      "Open in Photoshop",
      { type: "photoshop-media", target },
      unavailable || actions?.photoshop === false
    );
    if (!tile)
      action(
        "Open All in Photoshop",
        { type: "photoshop-all-media" },
        snapshot.photoshopAllAvailable === false
      );
  }
  if (snapshot.mediaActions)
    action(
      (actions?.fileAction ?? snapshot.mediaActions.fileAction) === "reveal" ? "Reveal in File Manager" : "Download",
      { type: "file-media", target },
      canvasMediaFile(snapshot, target) === null || actions?.file === false
    );
  if (comparisonCount > 0 && entries.length > comparisonCount)
    entries.splice(comparisonCount, 0, { label: "", separator: true });
  return entries;
}
function targetItem(snapshot, target) {
  if (target === "primary") return snapshot.selected;
  if (target === "secondary") return snapshot.secondary;
  return snapshot.grid.find((entry) => entry.source.id === target.sourceId)?.items.find((item) => item.id === target.itemId) ?? null;
}

export {
  canvasMediaFile,
  canvasContextActions
};
