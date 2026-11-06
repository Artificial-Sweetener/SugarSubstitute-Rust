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
/** Match SugarSubstitute stack metrics while inheriting official Fluent or native Comfy colors. */

import { MASONRY_STACK_Z_INDEX } from '../../core/OverlayStacking.js';
import { WORKFLOW_MASONRY_TOP_INSET } from '../masonry/WorkflowMasonryLayout.js';
import {
  CUBE_STACK_ROW_GAP,
  CUBE_STACK_ROW_HEIGHT,
  CUBE_STACK_ROW_INSET,
  cubeStackWidth,
} from './CubeStackMetrics.js';
import { packedMasonryRule } from '../masonry/MasonryPacking.js';
import {
  CUBE_MODEL_PILL_BACKGROUND,
  CUBE_MODEL_PILL_FOREGROUND,
} from '../CubeModelPillAppearance.js';

/** Install trusted static rules once without styling host nodes or discovering graph inventory. */
export function ensureCubeStackStyles(documentRef: Document): void {
  if (documentRef.getElementById('sugarcubes-cube-stack-styles')) return;
  const style = documentRef.createElement('style');
  style.id = 'sugarcubes-cube-stack-styles';
  style.textContent = `
    @property --sc-stack-wash-opacity { syntax:'<number>'; inherits:true; initial-value:1; }
    .sc-cube-stack {
      --sc-stack-bg:var(--comfy-menu-bg,#202020);
      --sc-stack-row:var(--secondary-background,#303030);
      --sc-stack-hover:var(--secondary-background-hover,#393939);
      --sc-stack-text:var(--fg-color,#eee);
      --sc-stack-muted:var(--descrip-text,#aaa);
      --sc-stack-border:var(--interface-stroke,rgba(128,128,128,.2));
      --sc-stack-accent:var(--sc-accent-indicator,var(--primary-color,#60cdff));
      --sc-stack-curve:cubic-bezier(.16,1,.3,1);
      box-sizing:border-box; width:224px; color:var(--sc-stack-text);
      font:14px/1.25 var(--sc-font-family,"Segoe UI Variable","Segoe UI",sans-serif);
    }
    .sc-cube-stack[data-appearance]:not([data-appearance="comfy"]) {
      --sc-stack-bg:var(--sc-surface-secondary,#202020);
      --sc-stack-row:var(--sc-surface,#303030);
      --sc-stack-hover:var(--sc-surface-hover,#393939);
      --sc-stack-text:var(--sc-text,#eee);
      --sc-stack-muted:var(--sc-text-muted,#aaa);
      --sc-stack-border:var(--sc-border-subtle,rgba(128,128,128,.2));
      --sc-stack-accent:var(--sc-accent-indicator,var(--sc-brand-fill,#60cdff));
      --sc-stack-curve:var(--sc-motion-decelerate,cubic-bezier(.16,1,.3,1));
    }
    .sc-context-menu[data-sc-cube-stack-menu][data-appearance="comfy"] {
      --sc-surface:var(--comfy-menu-bg,#202020) !important;
      --sc-surface-hover:var(--secondary-background,#303030) !important;
      --sc-text:var(--fg-color,#eee) !important;
      --sc-border:var(--interface-stroke,#555) !important;
    }
    .sc-cube-stack[data-compact] { width:${String(cubeStackWidth(true, false))}px; }
    .sc-cube-stack[data-kind="rail"] { transition:width 220ms var(--sc-stack-curve); }
    .sc-cube-stack__rows { position:relative; list-style:none; padding:0 ${String(CUBE_STACK_ROW_INSET)}px; margin:0; display:flex; flex-direction:column; gap:${String(CUBE_STACK_ROW_GAP)}px; }
    .sc-cube-stack[data-kind="rail"] .sc-cube-stack__rows > .sc-cube-stack-row {
      position:absolute; left:6px; right:6px; top:0; transition:transform 220ms var(--sc-stack-curve);
    }
    .sc-cube-stack-move { transition:transform 220ms var(--sc-stack-curve); }
    .sc-cube-stack-enter-active,.sc-cube-stack-leave-active { transition:opacity 160ms ease,transform 220ms var(--sc-stack-curve); }
    .sc-cube-stack-enter-from,.sc-cube-stack-leave-to { opacity:0; transform:translateY(8px); }
    .sc-cube-stack-leave-active { position:absolute; left:6px; right:6px; }
    .sc-cube-stack-row {
      position:relative; box-sizing:border-box; height:${String(CUBE_STACK_ROW_HEIGHT)}px; flex:0 0 ${String(CUBE_STACK_ROW_HEIGHT)}px;
      border:1px solid var(--sc-stack-border); border-radius:5px;
      background:var(--sc-stack-row); color:var(--sc-stack-text); list-style:none;
    }
    .sc-cube-stack-row:hover { background:var(--sc-stack-hover); }
    .sc-cube-stack[data-kind="rail"] .sc-cube-stack-row { background:transparent; border-color:transparent; }
    .sc-cube-stack[data-kind="rail"] .sc-cube-stack-row:hover { background:var(--sc-stack-hover); }
    .sc-cube-stack[data-kind="rail"] .sc-cube-stack-row[data-selected] { background:var(--sc-stack-row); border-color:var(--sc-stack-border); }
    .sc-cube-stack:not([data-kind="rail"]) .sc-cube-stack-row[data-selected]::before {
      content:""; position:absolute; left:0; top:14px; bottom:14px; width:3px;
      border-radius:2px; background:var(--sc-stack-accent); pointer-events:none;
    }
    .sc-cube-stack__selection { position:absolute; left:7px; top:0; width:3px; height:16px; border-radius:2px;
      background:var(--sc-stack-accent); pointer-events:none; z-index:2;
      transition:transform 220ms var(--sc-stack-curve),opacity 160ms ease; }
    .sc-cube-stack .sc-cube-stack-row[data-severity="error"] { background:color-mix(in srgb,#e81123 14%,var(--sc-stack-row)); border-color:color-mix(in srgb,#e81123 55%,var(--sc-stack-border)); }
    .sc-cube-stack .sc-cube-stack-row[data-severity="warning"] { background:color-mix(in srgb,#ffb900 12%,var(--sc-stack-row)); }
    .sc-cube-stack-row__activate {
      display:flex; align-items:center; gap:6px; width:100%; height:100%;
      padding:5px; margin:0; border:0; border-radius:inherit; background:transparent;
      color:inherit; font:inherit; text-align:left; cursor:pointer; min-width:0;
    }
    .sc-cube-stack-row__icon { position:relative; display:flex; width:34px; height:34px; flex:0 0 34px; align-items:center; justify-content:center; }
    .sc-cube-stack-row__asset { width:100%; height:100%; display:flex; align-items:center; justify-content:center; }
    /* Shared host icons carry a 24px flex basis; the rail and cart reserve 34px thumbnails. */
    .sc-cube-stack-row__asset > span { width:100%; height:100%; flex:0 0 100%; display:flex; align-items:center; justify-content:center; }
    .sc-cube-stack-row__icon img,.sc-cube-stack-row__icon canvas { width:100%; height:100%; max-width:none; max-height:none; object-fit:contain; }
    .sc-cube-stack-row__model {
      position:absolute; right:-3px; bottom:-2px; box-sizing:border-box; height:10px;
      min-width:10px; padding:0 2px; border-radius:5px; white-space:nowrap; pointer-events:none;
      display:flex; align-items:center; justify-content:center; background:${CUBE_MODEL_PILL_BACKGROUND};
      color:${CUBE_MODEL_PILL_FOREGROUND}; font:600 8px/10px var(--sc-font-family,"Segoe UI Variable","Segoe UI",sans-serif);
    }
    .sc-cube-stack-row[data-bypassed] .sc-cube-stack-row__asset { opacity:.42; }
    .sc-cube-stack-row__text { display:flex; flex-direction:column; gap:2px; min-width:0; flex:1; padding-right:30px; overflow:hidden;
      opacity:1; transform:translateX(0); transition:opacity 160ms ease,transform 220ms var(--sc-stack-curve); }
    .sc-cube-stack[data-compact] .sc-cube-stack-row__text { opacity:0; transform:translateX(-8px); pointer-events:none; }
    .sc-cube-stack-row__label,.sc-cube-stack-row__secondary { white-space:nowrap; overflow:hidden; text-overflow:ellipsis; }
    .sc-cube-stack-row__secondary { font-size:10px; color:var(--sc-stack-muted); }
    .sc-cube-stack-row__remove {
      position:absolute; right:6px; top:13px; width:18px; height:18px; padding:0;
      border:0; border-radius:4px; color:var(--sc-stack-muted); background:transparent;
      font:18px/1 "Segoe UI",sans-serif; cursor:pointer; opacity:0;
    }
    .sc-cube-stack-row:hover .sc-cube-stack-row__remove,.sc-cube-stack-row:focus-within .sc-cube-stack-row__remove { opacity:1; }
    .sc-cube-stack-row__remove:hover { background:var(--sc-stack-hover); color:var(--sc-stack-text); }
    .sc-cube-stack button:focus-visible { outline:2px solid var(--sc-stack-accent); outline-offset:2px; }
    .sc-cube-stack button:disabled { cursor:default; }
    .sc-cube-stack-row[data-reorderable],.sc-cube-stack-row[data-reorderable] .sc-cube-stack-row__activate { cursor:default; touch-action:none; user-select:none; }
    .sc-cube-stack[data-compact] .sc-cube-stack-row__activate { padding:5px; gap:0; }
    .sc-cube-stack[data-compact] .sc-cube-stack-row__icon { width:100%; height:100%; flex:0 0 100%; }
    .sc-cube-stack[data-compact] .sc-cube-stack-row__text { display:none; }
    .sc-cube-stack[data-compact] .sc-cube-stack-row__model { right:0; bottom:0; }
    .sc-cube-stack-row__icon { user-select:none; -webkit-user-drag:none; }
    .sc-cube-stack[data-pointer-dragging],.sc-cube-stack[data-pointer-dragging] * { cursor:default; user-select:none; }
    .sc-cube-stack[data-kind="rail"] .sc-cube-stack-row[data-dragging] {
      opacity:1; border:1px dashed color-mix(in srgb,var(--sc-stack-accent) 60%,transparent);
      background:color-mix(in srgb,var(--sc-stack-accent) 10%,var(--sc-stack-bg));
      box-shadow:inset 0 2px 6px #0003,0 0 0 1px color-mix(in srgb,var(--sc-stack-accent) 8%,transparent);
    }
    .sc-cube-stack[data-kind="rail"] .sc-cube-stack-row[data-dragging] > * { opacity:0; }
    .sc-cube-stack__drag-overlay { position:fixed; top:0; left:0; z-index:10; pointer-events:none; list-style:none; }
    .sc-cube-stack__drag-overlay .sc-cube-stack-row { opacity:1; background:var(--sc-stack-row) !important;
      border:1px solid var(--sc-stack-accent) !important; box-shadow:0 8px 24px #0005; transform:scale(1.035); }
    .sc-cube-stack__toolbar { display:flex; justify-content:flex-end; gap:2px; min-height:30px; padding:0 6px 4px; }
    .sc-cube-stack__tool { width:24px; height:24px; min-width:0; min-height:0; max-width:24px; flex:0 0 24px; padding:0; border:0; border-radius:4px; color:inherit; background:transparent; cursor:pointer; }
    .sc-cube-stack[data-compact] .sc-cube-stack__toolbar { padding-inline:4px; }
    .sc-cube-stack__tool:hover { background:var(--sc-stack-hover); }
    .sc-cube-stack__add { display:flex; justify-content:center; align-items:center; height:46px; margin:4px 6px 0; min-width:0; padding:0; box-sizing:border-box; width:calc(100% - 12px); border:1px dashed var(--sc-stack-border); border-radius:5px; background:transparent; color:var(--sc-stack-muted); cursor:pointer; font:22px/1 "Segoe UI",sans-serif; }
    .sc-cube-stack__add:hover { background:var(--sc-stack-hover); color:var(--sc-stack-text); }
    .sc-cube-stack__status { position:absolute; width:1px; height:1px; overflow:hidden; clip-path:inset(50%); white-space:nowrap; }
    .sc-cube-stack-host { position:absolute; z-index:${MASONRY_STACK_Z_INDEX}; pointer-events:auto; left:12px; top:${WORKFLOW_MASONRY_TOP_INSET}px; bottom:12px; overflow-y:auto; overflow-x:hidden; padding:6px 0; border-radius:8px; background:var(--comfy-menu-bg,#202020); }
    .sc-cube-stack-host[data-appearance]:not([data-appearance="comfy"]) { background:var(--sc-surface-secondary,#202020); border:1px solid var(--sc-border-subtle,transparent); box-shadow:var(--sc-shadow-small,0 2px 4px #0002); }
    /* Compact masonry and Plugin Mode pack the rail flush; the Cube stack's compact preference never does. */
    ${packedMasonryRule('.sc-cube-stack[data-kind="rail"][data-compact]')} { width:${String(cubeStackWidth(true, true))}px; }
    ${packedMasonryRule('.sc-cube-stack[data-kind="rail"] .sc-cube-stack__rows')} { gap:0; padding:0; }
    ${packedMasonryRule('.sc-cube-stack[data-kind="rail"] .sc-cube-stack__rows > .sc-cube-stack-row')} { left:0; right:0; }
    ${packedMasonryRule('.sc-cube-stack[data-kind="rail"] .sc-cube-stack-leave-active')} { left:0; right:0; }
    ${packedMasonryRule('.sc-cube-stack[data-kind="rail"] .sc-cube-stack__selection')} { left:1px; }
    ${packedMasonryRule('.sc-cube-stack[data-kind="rail"] .sc-cube-stack__add')} { margin:0; width:100%; border-radius:0; }
    ${packedMasonryRule('.sc-cube-stack[data-kind="rail"] .sc-cube-stack-row')} { border-radius:0; }
    ${packedMasonryRule('.sc-cube-stack-host')} { left:0; top:0; bottom:0; padding:0; border:0; border-radius:0; }
    @media (hover:none) { .sc-cube-stack-row__remove { opacity:1; } }
    @media (prefers-reduced-motion:reduce) {
      .sc-cube-stack-move,.sc-cube-stack-enter-active,.sc-cube-stack-leave-active,
      .sc-cube-stack[data-kind="rail"] .sc-cube-stack__rows > .sc-cube-stack-row,
      .sc-cube-stack[data-kind="rail"],.sc-cube-stack__selection,.sc-cube-stack-row__text { transition:none; }
    }
  `;
  documentRef.head.append(style);
}
