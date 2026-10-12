// frontend/comfyui/ui/output_canvas/domain/CanvasViewState.ts
function sameCanvasNavigation(previous, next) {
  return previous.viewId === next.viewId && previous.sessionId === next.sessionId && previous.follow === next.follow && previous.mode === next.mode && previous.sourcePreview === next.sourcePreview && previous.linked === next.linked && previous.selection?.sourceId === next.selection?.sourceId && previous.selection?.batchIndex === next.selection?.batchIndex && Boolean(previous.comparison) === Boolean(next.comparison) && previous.comparison?.secondary?.sourceId === next.comparison?.secondary?.sourceId && previous.comparison?.secondary?.batchIndex === next.comparison?.secondary?.batchIndex;
}

// frontend/comfyui/ui/output_canvas/presentation/vue/CanvasNavigationSnapshot.ts
function canvasNavigationSnapshot(current, previous) {
  return previous && previous.sources === current.sources && sameCanvasNavigation(previous.view, current.view) && previous.view.comparison?.orientation === current.view.comparison?.orientation && previous.selected?.id === current.selected?.id && previous.secondary?.id === current.secondary?.id && previous.preview === current.preview && sameResource(previous.resource, current.resource) && sameResource(previous.secondaryResource, current.secondaryResource) ? previous : current;
}
function sameResource(previous, current) {
  return previous?.state === current?.state && previous?.message === current?.message && previous?.canRetry === current?.canRetry && previous?.canAdmitLargeImage === current?.canAdmitLargeImage;
}

export {
  canvasNavigationSnapshot
};
