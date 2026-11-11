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
/** Translate GIMP dock, notebook and toolbutton materials without changing masonry layout ownership. */

/** Present cards as compact GTK panels and workflow tabs as GIMP notebook tabs. */
export function gimpWorkspaceCss(): string {
  const root = 'html[data-sc-visual-style="gimp"]';
  const fields = `${root} [data-sc-style-fields]:not(:is(.sc-overrides-toolbar,.sc-overrides-graph-content) *)`;
  return `
    ${fields} { display:grid; grid-template-columns:minmax(0,1fr); }
    ${fields} > [data-sc-style-row] { grid-column:1 / -1; grid-row:auto; padding-left:10px; padding-right:10px; }
    ${fields} > [data-sc-style-row][data-sc-style-column="2"]::before { display:none; }
    ${root} :is(.sugarcubes-workflow-masonry,.sugarcubes-workflow-masonry__backdrop,.sc-cube-stack-host) { background:var(--sc-gimp-background); font-size:12px; }
    ${root} .sugarcubes-workflow-masonry { --sugarcubes-masonry-gap:6; --sugarcubes-masonry-card-surface:var(--sc-gimp-background); }
    ${root}[data-sc-compact-masonry] .sugarcubes-workflow-masonry { --sugarcubes-masonry-gap:0; }
    ${root} .sugarcubes-workflow-masonry__section { border:1px solid var(--sc-gimp-border); border-radius:2px; padding:2px 6px; background:var(--sc-gimp-widget); }
    ${root} :is(.sugarcubes-workflow-masonry__section h2,.sc-overrides-card-title) { color:var(--sc-gimp-foreground); font:600 12px/18px var(--sc-font-family); }
    ${root} .sugarcubes-cube-face__node-card :is([data-testid^="node-header-"],[data-sc-style-node]) { border-radius:2px; }
    ${root} .sugarcubes-cube-face__node-card [data-testid^="node-header-"] { height:28px; min-height:28px; background:var(--sc-gimp-background); }
    ${root} .sugarcubes-cube-face__node-card [data-testid="node-title"] { font-size:12px; }
    ${root} :is(.sc-cube-stack,.sc-overrides-toolbar) { background:var(--sc-gimp-background); color:var(--sc-gimp-foreground); border-color:var(--sc-gimp-border); border-radius:2px; }
    ${root} .sc-overrides-toolbar[data-presentation="plugin"] > .sc-overrides-card-title { padding:5px 8px; font:600 12px/18px var(--sc-font-family); }
    ${root} .sc-overrides-toolbar[data-presentation="plugin"] > .sc-overrides-menu { width:14px; height:14px; min-width:0; min-height:0; padding:0; margin:0 8px; gap:0; border:0; background:transparent; }
    ${root} .sc-overrides-toolbar[data-presentation="plugin"] > .sc-overrides-menu > :first-child { width:14px; height:14px; }
    ${root} .sc-overrides-toolbar[data-presentation="plugin"] > .sc-overrides-menu > :last-child { display:none; }
    ${root} .sc-cube-stack-row { border-radius:2px; background:var(--sc-gimp-extreme); color:var(--sc-gimp-foreground); }
    ${root} .sc-cube-stack-row[data-selected] { background:var(--sc-gimp-extremeSelected); border-color:var(--sc-gimp-strongerBorder); }
    ${root} [data-sc-theme="gimp"].sc-substitute-frame { background:var(--sc-gimp-background); color:var(--sc-gimp-foreground); }
    ${root} :is([data-sc-plugin-toolbar],[data-sc-substitute-toolbar]) { background:var(--sc-gimp-background); border-radius:0; border-bottom:1px solid var(--sc-gimp-border); gap:2px; }
    ${root} :is([data-sc-plugin-toolbar],[data-sc-substitute-toolbar],.sugarcubes-workflow-masonry__actions) :is(button,[data-sc-button]):not(.sc-generation-strip *) { border:1px solid transparent; border-radius:3px; color:var(--sc-gimp-foreground); background:transparent; box-shadow:none; }
    ${root} :is([data-sc-plugin-toolbar],[data-sc-substitute-toolbar],.sugarcubes-workflow-masonry__actions) :is(button,[data-sc-button]):hover:not(:disabled) { background:var(--sc-gimp-selected); border-color:var(--sc-gimp-edgeBorder); }
    ${root} [data-sc-workflow-tabs] { display:flex; align-items:stretch; height:34px; min-width:0; width:var(--sc-workflow-tabs-width); overflow:clip; gap:2px; padding:1px 2px; }
    ${root} [data-sc-workflow-tab] { display:inline-flex; align-items:center; box-sizing:border-box; width:var(--sc-workflow-tab-width,180px); min-width:var(--sc-workflow-tab-width,180px); flex:0 0 var(--sc-workflow-tab-width,180px); height:30px; padding:1px 6px 1px 20px; border:1px solid var(--sc-gimp-strongBorder); border-radius:2px; background:var(--sc-gimp-background); font:12px/18px var(--sc-font-family); }
    ${root} [data-sc-workflow-tab][aria-selected="true"] { background:var(--sc-gimp-selected); box-shadow:inset 0 -4px var(--sc-gimp-strongerBorder); }
    ${root} [data-sc-workflow-tab][data-orb-adjacent] { padding-left:32px; }
    ${root} .sc-generation-strip { --sc-generation-backdrop:var(--sc-gimp-background); }
    ${root} :is(.sc-generation-batch,.sc-generation-actions) { background:var(--sc-gimp-widget); color:var(--sc-gimp-foreground); border-radius:3px; box-shadow:inset 0 1px var(--sc-gimp-extremeSelected); }
    ${root} .sc-generation-action { color:var(--sc-gimp-foreground); }
    ${root} .sc-generation-action:disabled { background:var(--sc-gimp-widget); color:var(--sc-gimp-disabled); }
    ${root}[data-sc-substitute-frame] button[data-sc-generation-queue] { background:var(--sc-gimp-widget)!important; color:var(--sc-gimp-foreground)!important; border-radius:0 3px 3px 0!important; }
    ${root}[data-sc-substitute-frame] button[data-sc-generation-queue]:hover { background:var(--sc-gimp-hover)!important; }
  `;
}
