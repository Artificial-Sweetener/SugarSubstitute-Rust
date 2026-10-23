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
/** Own PhotoPro workspace materials and Photoshop-style tool button presentation. */

/** Share toolbar and cube tool states while retaining generation's segmented control. */
export function photoProWorkspaceCss(): string {
  const tools =
    'html[data-sc-visual-style="photopro"] :is(:is([data-sc-plugin-toolbar],[data-sc-substitute-toolbar]) :is(button,[data-sc-button]):where(:not(.sc-generation-strip *,[data-sc-generation-queue])),.sugarcubes-workflow-masonry__section .sugarcubes-workflow-masonry__icon-action)';
  const cubes =
    'html[data-sc-visual-style="photopro"] .sc-cube-stack[data-kind="rail"] :is(.sc-cube-stack__rows,.sc-cube-stack__drag-overlay) > .sc-cube-stack-row:where(:not([data-severity="error"],[data-severity="warning"]))';
  const add = 'html[data-sc-visual-style="photopro"] .sc-cube-stack .sc-cube-stack__add';
  return `
    html[data-sc-visual-style="photopro"] :is(.sugarcubes-workflow-masonry,.sugarcubes-workflow-masonry__backdrop,.sc-cube-stack-host),
    html[data-sc-visual-style="photopro"] #graph-canvas-container[data-sugarcubes-masonry="true"] { background:var(--sc-photopro-workspace-fill); }
    html[data-sc-visual-style="photopro"][data-sc-substitute-frame] :is([data-sc-substitute-toolbar-menu],[data-sc-substitute-toolbar-line],[data-sc-plugin-toolbar]) { background:var(--sc-photopro-workspace-fill); }
    html[data-sc-visual-style="photopro"][data-sc-substitute-frame] [data-sc-substitute-toolbar-line] { height:auto; min-height:35px; }
    html[data-sc-visual-style="photopro"][data-sc-substitute-frame] .p-splitter:has([data-sc-substitute-toolbar]) { border-top-left-radius:0; border-top-right-radius:0; }
    html[data-sc-visual-style="photopro"] [data-sc-substitute-toolbar][data-sc-plugin-toolbar] { height:auto; min-height:35px; padding:3px 4px; gap:2px; background:var(--sc-photopro-workspace-fill); border-bottom:1px solid var(--sc-border); }
    html[data-sc-visual-style="photopro"] .sc-plugin-toolbar-start { gap:2px; }
    html[data-sc-visual-style="photopro"] .sugarcubes-workflow-masonry__actions { gap:2px; padding-right:4px; }
    html[data-sc-visual-style="photopro"] [data-sc-plugin-toolbar] > .sc-generation-strip { margin-left:auto; }
    ${tools},${cubes},${add} { box-sizing:border-box; border:1px solid transparent; border-radius:0; background:transparent; box-shadow:none; color:var(--sc-photopro-tool-foreground); }
    ${tools} { width:28px; height:28px; min-width:28px; min-height:28px; padding:0; flex:none; }
    ${tools} :is(i,svg,.sc-application-gear) { width:14px; height:14px; font-size:14px; }
    html[data-sc-visual-style="photopro"] [data-sc-plugin-toolbar] > :has(button > i:is([class~="icon-[lucide--panel-right]"],[class~="icon-[lucide--panel-left]"])) { width:28px; height:28px; }
    ${tools}:is(:hover,:focus-visible),${cubes}:is(:hover,:focus-within),${add}:is(:hover,:focus-visible) { background:var(--sc-photopro-tool-hover-fill); border-color:var(--sc-photopro-tool-hover-border); }
    ${tools}:is(:hover,:focus-visible) { border-radius:4px; }
    ${tools}:is(:active,[aria-pressed="true"],[aria-expanded="true"],[aria-checked="true"]),${cubes}[data-selected] { background:var(--sc-photopro-tool-selected-fill); border-color:var(--sc-photopro-tool-selected-border); }
    html[data-sc-visual-style="photopro"] :is(.sc-generation-batch,.sc-generation-actions) { border-radius:0; }
    html[data-sc-visual-style="photopro"] .sc-generation-strip { height:28px; --sc-generation-backdrop:var(--sc-photopro-workspace-fill); }
    html[data-sc-visual-style="photopro"] :is(.sc-generation-batch,.sc-generation-actions,.sc-generation-action,.sc-generation-queue-slot) { height:28px; }
    html[data-sc-visual-style="photopro"] .sc-generation-batch input { height:24px; font-size:12px; }
    html[data-sc-visual-style="photopro"] .sc-generation-step { height:14px; }
    html[data-sc-visual-style="photopro"] .sc-generation-step[data-direction="down"] { top:14px; }
    html[data-sc-visual-style="photopro"] :is(.sc-generation-glyph,.sc-generation-queue-glyph) { width:14px; height:14px; }
    html[data-sc-visual-style="photopro"] .sc-generation-action[data-action="skip"] .sc-generation-glyph { width:18px; height:18px; }
    html[data-sc-visual-style="photopro"] .sc-generation-strip .sc-generation-action:disabled { background:var(--sc-generation-batch-fill); }
    html[data-sc-visual-style="photopro"][data-sc-substitute-frame] button[data-sc-generation-queue] { height:28px!important; border-radius:0!important; }
  `;
}
