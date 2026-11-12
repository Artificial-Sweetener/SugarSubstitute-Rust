//    SugarCubes - composable workflow units for ComfyUI
//    Copyright (C) 2026  Artificial Sweetener and contributors
//
//    This program is free software: you can redistribute it and/or modify
//    it under the terms of the GNU Affero General Public License as published by
//    the Free Software Foundation, either version 3 of the License, or
//    (at your option) any later version.
//
//    This program is distributed in the hope that it will be useful,
//    but WITHOUT ANY WARRANTY; without even the implied warranty of
//    MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
//    GNU Affero General Public License for more details.
//
//    You should have received a copy of the GNU Affero General Public License
//    along with this program.  If not, see <https://www.gnu.org/licenses/>.
/** Transport native editor tooltip coordinates through SugarCubes' shared popup owner. */
import { createFloatingPlane } from '../../menus/FloatingSurface.js';
import type { FloatingPlane } from '../../menus/FloatingSurface.js';
import type { TooltipTransport } from '../../../../sugarprompt/editor/src/tooltip-transport.js';

/** CodeMirror positions its children; FloatingSurface alone owns browser top-layer state. */
export class SugarPromptTooltip implements TooltipTransport {
  readonly #surface: FloatingPlane;

  /** Capture one native tooltip plane without duplicating geometry or focus state. */
  constructor(plane: HTMLElement) {
    this.#surface = createFloatingPlane(plane);
  }

  /** Publish the exact native plane without repositioning its measured tooltip children. */
  show(): void {
    this.#surface.show();
  }

  /** Release top-layer membership when native tooltip state becomes empty. */
  hide(): void {
    this.#surface.hide();
  }

  /** Retire shared listeners and browser transport before native teardown completes. */
  dispose(): void {
    this.#surface.dispose();
  }
}
