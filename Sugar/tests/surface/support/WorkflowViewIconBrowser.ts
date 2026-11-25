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
/** The graph/masonry toggle's icon in its real toolbar button, posed at any animation position. */
import { ensureWorkflowMasonryStyles } from '../../../frontend/comfyui/ui/surface/masonry/WorkflowMasonryStyles.js';
import { WorkflowViewIcon } from '../../../frontend/comfyui/ui/surface/masonry/WorkflowViewIcon.js';

declare global {
  interface Window {
    workflowViewIconProof: {
      /** Draw the icon at `position`, 0 the graph and 1 the masonry. */
      pose(position: number): void;
      /** The icon's box in CSS pixels, relative to the stage. */
      box(): { left: number; top: number; width: number; height: number };
    };
  }
}

ensureWorkflowMasonryStyles(document);
const style = document.createElement('style');
style.textContent = `
  body { margin:0; background:#fff; }
  #stage { position:relative; width:80px; height:80px; background:#fff; }
  /* Black ink on white, with nothing of the button's own drawn, so every dark pixel is the icon's. */
  #stage .sugarcubes-masonry-toggle { position:absolute; left:24px; top:24px; width:32px; height:32px; padding:0;
    border:0; background:transparent; box-shadow:none; color:#000; display:flex; align-items:center; justify-content:center; }`;
document.head.append(style);
const stage = document.createElement('div');
stage.id = 'stage';
const button = document.createElement('button');
button.className = 'sugarcubes-masonry-toggle';
const holder = document.createElement('span');
holder.className = 'sugarcubes-masonry-toggle__icon';
const icon = new WorkflowViewIcon(document, 0);
holder.append(icon.element);
button.append(holder);
stage.append(button);
document.body.append(stage);

window.workflowViewIconProof = {
  pose: (position) => icon.pose(position),
  box() {
    const bounds = icon.element.getBoundingClientRect();
    const origin = stage.getBoundingClientRect();
    return {
      left: bounds.left - origin.left,
      top: bounds.top - origin.top,
      width: bounds.width,
      height: bounds.height,
    };
  },
};
