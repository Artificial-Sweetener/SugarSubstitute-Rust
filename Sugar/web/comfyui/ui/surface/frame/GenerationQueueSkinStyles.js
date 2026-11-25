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
/**
 * Make room for the generation queue's own view where ComfyUI shows its job queue.
 *
 * Inside the SugarSubstitute frame the queue is drawn by the frame (see `queue/`). These rules keep
 * ComfyUI's own fly-out out of sight and turn its pinned side panel into the place beside the
 * canvas that holds the frame's pinned queue: at the right, at the fly-out's width. It is not
 * resized: Comfy's splitter edge beside it is hidden.
 */
import { GENERATION_QUEUE_DOCK, GENERATION_QUEUE_DOCKED_ATTRIBUTE } from './GenerationQueueDock.js';
import { GENERATION_QUEUE_FLYOUT_WIDTH } from './queue/GenerationQueueViewStyles.js';
/** ComfyUI's queue fly-out. */
const FLYOUT = ':root[data-sc-substitute-frame] [data-testid="queue-progress-overlay"]';
/** The side panel ComfyUI gives its pinned job panel. */
const DOCK_PANEL = `:root[data-sc-substitute-frame] .side-bar-panel:has(${GENERATION_QUEUE_DOCK})`;
/** The page while the queue is docked, as the dock's owner marks it on the root. */
const DOCKED = `:root[data-sc-substitute-frame][${GENERATION_QUEUE_DOCKED_ATTRIBUTE}]`;
/** Mix the text colour into transparency, which reads right on dark and light surfaces. */
const tint = (percent) => `color-mix(in srgb,var(--sc-text,#fff) ${String(percent)}%,transparent)`;
/** The rules that hand ComfyUI's queue surfaces to the frame's own queue. */
export const generationQueueSkinCss = `
  /* The frame shows the generation queue's own view; ComfyUI's panels stay out of sight. */
  ${FLYOUT} { display:none!important; }
  ${DOCK_PANEL} > :not(.sc-generation-queue) { display:none!important; }
  /* Docked, the queue is Comfy's side panel: the frame shows that one panel, pinned at the right at
     the fly-out's width, and the queue button steps aside so the queue is never offered in two places. */
  ${DOCK_PANEL} { --sc-queue-dock:${String(GENERATION_QUEUE_FLYOUT_WIDTH)}px;
    display:flex!important; order:3; flex:0 0 var(--sc-queue-dock)!important; position:relative;
    width:var(--sc-queue-dock); min-width:0!important; box-sizing:border-box;
    background:var(--sc-surface-secondary,#2b2b2b)!important; border-left:1px solid var(--sc-border-subtle,${tint(8.6)});
    color:var(--sc-text,#fff); font-family:var(--sc-font-family); }
  ${DOCKED} :is(button[data-sc-generation-queue],.sc-generation-queue-slot) {
    display:none!important; }
  ${DOCKED} .sc-generation-strip { justify-content:flex-end; }
  ${DOCKED} .sc-generation-actions { width:120px; }
  /* The docked queue is not resized. Comfy's splitter keeps its resizing edge after the panel,
     which the panel's move to the right leaves at the far left of the workspace. */
  ${DOCKED} .side-bar-panel + .p-splitter-gutter { display:none!important; }
`;
