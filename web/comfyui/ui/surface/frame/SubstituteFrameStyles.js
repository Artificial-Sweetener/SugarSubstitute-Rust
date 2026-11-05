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
/** Scope the reference shell geometry to the explicitly selected Substitute visual mode. */
import { FRAME_TAB_START, WORKFLOW_TAB_TOOL_WIDTH, WORKFLOW_NEW_BUTTON_GAP, } from './WorkflowTabStripLayout.js';
import { SPACED_MASONRY_ROOT } from '../masonry/MasonryPacking.js';
/** Keep browser tabs legible on black; let the desktop host supply native frame background and foreground. */
export const substituteFrameCss = `
  .sc-generation-progress { position:absolute; left:0; right:0; top:calc(100% - 3px); height:6px;
    display:flex; flex-direction:column; gap:0; pointer-events:none; z-index:3; background:transparent; }
  .sc-generation-progress[hidden] { display:none; }
  .sc-generation-progress > [role="progressbar"] { flex:0 0 3px; height:3px; border-radius:2px;
    transition:width 150ms ease-out; background:var(--sc-accent-color); }
  .sc-generation-progress > .sc-generation-progress__sampler { background:#f59e0b; }
  .sc-substitute-frame[data-color-scheme="light"] .sc-generation-progress > .sc-generation-progress__sampler { background:#c56a00; }
  :root[data-sc-substitute-frame] [data-testid="queue-inline-progress"],
  :root[data-sc-substitute-frame] :has(> [data-testid="linear-progress-overall"]) { display:none !important; }
  :root[data-sc-substitute-frame]:has([data-sc-generation-queue][aria-pressed="false"])
    [data-testid="queue-progress-overlay"] { display:none !important; }
  @media (prefers-reduced-motion:reduce) { .sc-generation-progress > [role="progressbar"] { transition:none; } }
  @property --sc-tab-cutout-radius { syntax:'<length>'; inherits:false; initial-value:0px; }
  @property --sc-tab-cutout-x { syntax:'<length>'; inherits:false; initial-value:-17px; }
  .sc-substitute-frame [data-sc-workflow-tab] { transition:--sc-tab-cutout-radius 160ms cubic-bezier(.645,.045,.355,1); }
  .sc-substitute-frame [data-sc-workflow-tab] { touch-action:none; user-select:none; }
  .sc-substitute-frame [data-sc-workflow-tab][data-sc-held] { z-index:3; cursor:grabbing; }
  .sc-substitute-frame [data-sc-tab-dragging] { cursor:grabbing; }
  @media(prefers-reduced-motion:reduce) { .sc-substitute-frame [data-sc-workflow-tab] { transition:none !important; } }
  @media(prefers-reduced-motion:reduce) { .sc-substitute-frame [data-sc-workflow-tab] { transition:none; } }
  :root[data-sc-substitute-frame] { --workflow-tabs-height:34px; }
  :root[data-sc-substitute-frame] [data-sc-frame-native] { display:none !important; }
  :root[data-sc-tab-menu-capture] [role='menu'] { visibility:hidden!important; }
  :root[data-sc-substitute-frame] :is(.side-toolbar-container,.side-tool-bar-container,.side-bar-panel) { display:none !important; }
  :root[data-sc-substitute-frame] .splitter-overlay:not(.splitter-overlay-bottom) > .p-splitter-gutter { display:none !important; }
  :root[data-sc-substitute-frame] .subgraph-breadcrumb { padding-left:62px; }
  :root[data-sc-substitute-frame] div:has(> .subgraph-breadcrumb) { min-width:190px; }
  .sc-substitute-frame { position:absolute; inset:0; height:34px; display:flex; padding:4px 0 0 ${String(FRAME_TAB_START)}px;
    box-sizing:border-box; align-items:flex-end; z-index:1000; pointer-events:auto;
    background:var(--sc-native-frame-background,#000); color:var(--sc-text); font-family:var(--sc-font-family);
    --sc-frame-muted:var(--sc-native-frame-foreground,rgba(255,255,255,.788));
    --sc-frame-hover:rgba(255,255,255,.059); --sc-frame-pressed:rgba(255,255,255,.047);
    --sc-frame-unread:color-mix(in srgb,var(--sc-accent-color) 17.25%,transparent);
    --sc-frame-shimmer-edge:rgba(255,255,255,.063); --sc-frame-shimmer-center:rgba(255,255,255,.176); }
  .sc-substitute-frame[data-color-scheme='light'] {
    --sc-frame-hover:color-mix(in srgb,var(--sc-native-frame-foreground,#fff) 3.9%,transparent);
    --sc-frame-pressed:color-mix(in srgb,var(--sc-native-frame-foreground,#fff) 2.7%,transparent);
    --sc-frame-unread:color-mix(in srgb,var(--sc-accent-color) 11%,transparent);
    --sc-frame-shimmer-edge:rgba(255,255,255,.086); --sc-frame-shimmer-center:rgba(255,255,255,.216); }
  .sc-substitute-frame .sc-workflow-title { display:block; white-space:nowrap; min-width:0; overflow:hidden;
    mask-image:linear-gradient(90deg,#000 calc(100% - 10px),transparent); }
  .sc-substitute-frame .sc-workflow-shimmer { position:absolute; top:0; bottom:0; left:-36px; width:clamp(24px,32%,36px);
    background:linear-gradient(90deg,var(--sc-frame-shimmer-edge),var(--sc-frame-shimmer-center),var(--sc-frame-shimmer-edge));
    pointer-events:none; opacity:0; }
  .sc-substitute-frame [data-sc-workflow-tab][data-shimmer] .sc-workflow-shimmer { animation:sc-frame-result-shimmer 800ms cubic-bezier(.215,.61,.355,1) forwards; }
  @keyframes sc-frame-result-shimmer { 0% { left:-36px; opacity:1; } 85% { opacity:1; } 100% { left:100%; opacity:0; } }
  @media(prefers-reduced-motion:reduce) { .sc-substitute-frame [data-sc-workflow-tab][data-shimmer] .sc-workflow-shimmer { animation:none; } }
  .sc-substitute-frame .sc-workflow-close { display:flex; align-items:center; justify-content:center; flex:0 0 18px;
    width:18px; height:18px; min-width:18px; padding:0; border-radius:4px; visibility:hidden; color:inherit; }
  .sc-substitute-frame [data-sc-workflow-tab]:hover .sc-workflow-close,
  .sc-substitute-frame [data-sc-workflow-tab]:focus-within .sc-workflow-close { visibility:visible; }
  .sc-substitute-frame [data-sc-workflow-tab][data-modified] .sc-workflow-title::after { content:' •'; }
  .sc-substitute-frame .sc-frame-tool { flex:0 0 30px; width:30px; height:30px; min-width:0; padding:0; border-radius:0; }
  .sc-substitute-frame :is(.sc-workflow-new,.sc-workflow-overflow,.sc-workflow-scroll) { color:var(--sc-native-frame-foreground,#fff); }
  .sc-substitute-frame :is(.sc-workflow-new,.sc-workflow-overflow,.sc-workflow-scroll):hover { background:var(--sc-frame-hover); }
  .sc-substitute-frame :is(.sc-workflow-new,.sc-workflow-overflow,.sc-workflow-scroll):active { background:var(--sc-frame-pressed); }
  .sc-substitute-frame .sc-workflow-strip { display:flex; align-items:center; height:30px; min-width:0;
    flex:0 0 auto; overflow-x:auto; overflow-y:hidden; scrollbar-width:none; }
  .sc-substitute-frame .sc-workflow-strip::-webkit-scrollbar { display:none; }
  .sc-substitute-frame :is(.sc-workflow-new,.sc-workflow-overflow,.sc-workflow-scroll) { width:${String(WORKFLOW_TAB_TOOL_WIDTH)}px;
    flex:0 0 ${String(WORKFLOW_TAB_TOOL_WIDTH)}px; height:24px; min-height:24px; align-self:center; border-radius:4px; }
  .sc-substitute-frame .sc-workflow-scroll::before { content:''; display:block; width:6px; height:6px; border:solid currentColor; border-width:0 1.5px 1.5px 0; transform:rotate(-45deg); }
  .sc-substitute-frame .sc-workflow-scroll--previous::before { transform:rotate(135deg); }
  .sc-substitute-frame .sc-workflow-new { position:relative; margin-left:${String(WORKFLOW_NEW_BUTTON_GAP)}px; }
  .sc-substitute-frame .sc-workflow-new-glyph { display:block; width:12px; height:12px;
    flex:none; overflow:visible; pointer-events:none; }
  .sc-substitute-frame .sc-workflow-overflow { margin-left:0; }
  .sc-substitute-frame .sc-workflow-overflow[hidden],.sc-substitute-frame .sc-workflow-scroll[hidden] { display:none !important; }
  .sc-substitute-frame .sc-app-orb { position:absolute; left:8px; top:6px; width:46px; height:46px; min-width:46px;
    padding:0; border:0; border-radius:50%; background:transparent; overflow:hidden; clip-path:circle(50%); z-index:2; }
  .sc-substitute-frame .sc-app-orb canvas { display:block; width:46px; height:46px; pointer-events:none; }
  .sc-substitute-frame .sc-orb-actions { position:absolute; top:38px; left:8px; display:flex; gap:1px; width:47px; height:36px; }
  .sc-substitute-frame .sc-orb-actions[hidden] { display:none; }
  .sc-substitute-frame .sc-orb-actions::after { content:''; position:absolute; left:23px; top:20px; height:10px; width:1px; background:var(--sc-border-subtle); }
  .sc-substitute-frame .sc-orb-actions [data-sc-button] { position:relative; width:23px; height:36px; flex:0 0 23px; min-width:0; padding:0; border-radius:5px; mask-image:radial-gradient(circle 25px at var(--sc-orb-center-x,23px) -9px,transparent 24px,#000 25px); }
  .sc-substitute-frame .sc-orb-actions [data-sc-button]:last-child { --sc-orb-center-x:-1px; }
  .sc-substitute-frame :is(.sc-orb-stack-icon,.sc-orb-pin-icon) { display:block; width:13px; height:13px; background:currentColor; mask:var(--sc-orb-glyph) center/contain no-repeat; }
  .sc-substitute-frame .sc-orb-pin-icon { width:11px; height:11px; }
  .sc-substitute-frame .sc-orb-stack-icon { position:absolute; left:5px; top:18.5px; }
  .sc-substitute-frame .sc-orb-pin-icon { position:absolute; left:6px; top:17px; }
  .sc-substitute-frame .sc-orb-chevron { position:absolute; left:10px; top:29px; width:3.5px; height:3.5px; border:solid currentColor; border-width:0 1.1px 1.1px 0; transform:rotate(45deg); }
  ${SPACED_MASONRY_ROOT}[data-sc-substitute-frame] .sc-cube-stack-host { top:6px; bottom:6px; }
  :root[data-sc-substitute-frame] .sc-cube-stack-host { left:0; padding:0; border:0; border-radius:0;
    background:transparent; box-shadow:none; scrollbar-width:none; }
  :root[data-sc-substitute-frame] .splitter-overlay-bottom:has(> .graph-canvas-panel) { margin-left:0; margin-right:0; margin-bottom:0; }
  :root[data-sc-substitute-frame] .sc-cube-stack-host::-webkit-scrollbar { display:none; }
  :root[data-sc-substitute-frame] .sc-cube-stack[data-kind='rail'] .sc-cube-stack__toolbar { display:none; }
  ${SPACED_MASONRY_ROOT}[data-sc-substitute-frame] .sc-cube-stack-row { border-radius:4px; }
  :root[data-sc-substitute-frame] .sc-cube-stack-row__text { mask-image:linear-gradient(90deg,#000 calc(100% - 10px),transparent); }
  :root[data-sc-substitute-frame] .sc-cube-stack-row__label { text-overflow:clip; }
  :root[data-sc-substitute-frame] .sc-cube-stack[data-kind='rail'] .sc-cube-stack-row:hover { background:rgba(255,255,255,.059); }
  :root[data-sc-substitute-frame] .sc-cube-stack[data-kind='rail'] .sc-cube-stack-row:active { background:rgba(255,255,255,.047); }
  :root[data-sc-substitute-frame] .sc-cube-stack-host[data-color-scheme='light'] .sc-cube-stack[data-kind='rail'] .sc-cube-stack-row:hover { background:rgba(0,0,0,.039); }
  :root[data-sc-substitute-frame] .sc-cube-stack-host[data-color-scheme='light'] .sc-cube-stack[data-kind='rail'] .sc-cube-stack-row:active { background:rgba(0,0,0,.027); }
  :root[data-sc-substitute-frame] .sc-cube-stack[data-kind='rail'][data-compact] .sc-cube-stack-row__remove { display:none; }
`;
