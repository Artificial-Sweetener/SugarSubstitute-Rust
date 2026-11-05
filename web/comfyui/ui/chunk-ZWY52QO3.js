import {
  CANVAS_EDIT_TEXT
} from "./chunk-2WJX4EBF.js";

// frontend/comfyui/ui/canvas_editing/presentation/CanvasEditContextMenu.ts
import { ContextMenu } from "./menus/ContextMenu.js";
var CanvasEditContextMenu = class {
  constructor(region, target) {
    this.region = region;
    this.target = target;
    this.#menu = new ContextMenu(region.ownerDocument);
    region.addEventListener("contextmenu", this.#open);
  }
  #menu;
  /** Close any open menu whose state may no longer apply. */
  close() {
    this.#menu.close();
  }
  dispose() {
    this.region.removeEventListener("contextmenu", this.#open);
    this.#menu.close();
  }
  #open = (event) => {
    const target = this.target();
    const snapshot = target?.read();
    if (!target || !snapshot) return;
    event.preventDefault();
    event.stopPropagation();
    if (snapshot.surface.kind === "coverage") return;
    this.#menu.open({
      label: CANVAS_EDIT_TEXT.canvasActions,
      anchor: { kind: "pointer", x: event.clientX, y: event.clientY },
      trigger: this.region,
      entries: [
        {
          label: CANVAS_EDIT_TEXT.editLayerCoverage,
          disabled: snapshot.state.activeMask === null,
          attributes: { "data-canvas-action": "edit-layer-coverage" },
          select: () => target.openCoverage()
        }
      ]
    });
  };
};

export {
  CanvasEditContextMenu
};
