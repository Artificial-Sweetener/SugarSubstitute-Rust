// frontend/comfyui/ui/prompt_editor/runtime/SugarPromptTooltip.ts
import { createFloatingPlane } from "../../menus/FloatingSurface.js";
var SugarPromptTooltip = class {
  #surface;
  /** Capture one native tooltip plane without duplicating geometry or focus state. */
  constructor(plane) {
    this.#surface = createFloatingPlane(plane);
  }
  /** Publish the exact native plane without repositioning its measured tooltip children. */
  show() {
    this.#surface.show();
  }
  /** Release top-layer membership when native tooltip state becomes empty. */
  hide() {
    this.#surface.hide();
  }
  /** Retire shared listeners and browser transport before native teardown completes. */
  dispose() {
    this.#surface.dispose();
  }
};

export {
  SugarPromptTooltip
};
