// frontend/comfyui/ui/canvas_editing/domain/CanvasToolbarPlacement.ts
var CANVAS_EDIT_TOOLBAR_GAP = 12;
var CANVAS_EDIT_CHROME_INSET = 8;
var EDGE_HYSTERESIS_PHYSICAL_PX = 24;
var CanvasToolbarPlacement = class {
  #side = null;
  /** Forget the retained side so the next context is placed afresh. */
  reset() {
    this.#side = null;
  }
  /** Project the toolbar's top-left point for one context, size and safe area. */
  position(context, size, safe, physicalPixelsPerCssPixel) {
    const side = context ? this.#resolve(context, safe, physicalPixelsPerCssPixel) : null;
    this.#side = side;
    const bottom = safe.top + safe.height;
    if (!context || side === "viewport-bottom")
      return clamp(
        { x: safe.left + safe.width / 2 - size.width / 2, y: bottom - size.height },
        size,
        safe
      );
    const x = context.left + context.width / 2 - size.width / 2;
    const y = side === "above" ? context.top - CANVAS_EDIT_TOOLBAR_GAP - size.height : context.top + context.height + CANVAS_EDIT_TOOLBAR_GAP;
    return clamp({ x, y }, size, safe);
  }
  #resolve(context, safe, density) {
    const safeBottom = safe.top + safe.height;
    const contextBottom = context.top + context.height;
    if (context.top < safe.top && contextBottom > safeBottom) return "viewport-bottom";
    const overflow = (contextBottom - safeBottom) * Math.max(1, density);
    if (this.#side === "above") return overflow <= -EDGE_HYSTERESIS_PHYSICAL_PX ? "below" : "above";
    return overflow > EDGE_HYSTERESIS_PHYSICAL_PX ? "above" : "below";
  }
};
function canvasEditSafeArea(viewport) {
  return {
    left: viewport.left + CANVAS_EDIT_CHROME_INSET,
    top: viewport.top + CANVAS_EDIT_CHROME_INSET,
    width: Math.max(0, viewport.width - 2 * CANVAS_EDIT_CHROME_INSET),
    height: Math.max(0, viewport.height - 2 * CANVAS_EDIT_CHROME_INSET)
  };
}
function clamp(point, size, safe) {
  const maxX = Math.max(safe.left, safe.left + safe.width - size.width);
  const maxY = Math.max(safe.top, safe.top + safe.height - size.height);
  return {
    x: Math.round(Math.min(Math.max(point.x, safe.left), maxX)),
    y: Math.round(Math.min(Math.max(point.y, safe.top), maxY))
  };
}

export {
  CANVAS_EDIT_CHROME_INSET,
  CanvasToolbarPlacement,
  canvasEditSafeArea
};
