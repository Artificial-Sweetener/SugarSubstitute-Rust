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
/** Match Substitute's custom QFluent tab painter within official Microsoft components. */
/** Share the scrollable reference tab stride between styling and orb-adjacency projection. */
export const WORKFLOW_TAB_WIDTH = 180;
/** Preserve readable titles and the close target before enabling overflow. */
export const WORKFLOW_TAB_MIN_WIDTH = 120;
/** Leave three pixels of clearance around the 46-pixel orb. */
export const WORKFLOW_TAB_ORB_CUTOUT_RADIUS = 26;
/** Scope the reference tab body, accent and cutout to workflow-specific Microsoft controls. */
export const workflowTabCss = `
  :host { height:30px; width:var(--sc-workflow-tab-width,180px); min-width:var(--sc-workflow-tab-width,180px); max-width:var(--sc-workflow-tab-width,180px); flex:0 0 var(--sc-workflow-tab-width,180px); padding:0 8px 0 20px;
    border-radius:8px 8px 0 0; gap:6px; font:13px/20px var(--fontFamilyBase); overflow:hidden;
    color:var(--sc-frame-muted); background:transparent; box-shadow:none; }
  :host .tab-content { min-width:0; flex:1; overflow:hidden; padding:0; }
  :host .tab-content::after { display:none; }
  :host([aria-selected='true']) { background:var(--comfy-menu-bg,#202020); color:var(--colorNeutralForeground1); font-weight:600; }
  :host(:not([aria-selected='true']):hover) { background:var(--sc-frame-hover); }
  :host(:not([aria-selected='true'])) { clip-path:inset(1px 1px 0 round 7px 7px 0 0); }
  :host(:not([aria-selected='true']):active) { background:var(--sc-frame-pressed); }
  :host([data-unread]:not([aria-selected='true'])) { background:var(--sc-frame-unread); }
  :host([data-sc-held]:not([aria-selected='true'])) { background:linear-gradient(var(--sc-frame-hover),var(--sc-frame-hover)),var(--sc-tab-drag-backdrop,var(--colorNeutralBackground1)); }
  :host::after { content:''; position:absolute; inset:0 0 auto; height:2px; width:auto; border-radius:0;
    background:transparent; transform:none; transition:none; }
  :host([aria-selected='true'])::after { background:var(--sc-accent-indicator); }
  :host { --sc-tab-cutout-radius:0px; mask-image:radial-gradient(circle var(--sc-tab-cutout-radius) at var(--sc-tab-cutout-x,-17px) 25px,transparent calc(var(--sc-tab-cutout-radius) - 1px),#000 var(--sc-tab-cutout-radius)); }
  :host([data-orb-adjacent]) { padding-left:32px; --sc-tab-cutout-radius:${String(WORKFLOW_TAB_ORB_CUTOUT_RADIUS)}px; }
`;
/** Suppress Microsoft's bottom indicator while retaining its focus and selection machinery. */
export const workflowTablistCss = `
  :host::after { content:none !important; display:none !important; }
  :host { height:30px; min-width:0; width:var(--sc-workflow-tabs-width); gap:0; flex:0 0 auto; overflow:clip; }
  :host ::slotted([slot='tab']) { padding:0 8px 0 20px; }
  :host ::slotted([slot='tab'][data-orb-adjacent]) { padding-left:32px; }
  :host ::slotted([slot='tab'])::before { content:none; }
  :host ::slotted([slot='tab'])::after { inset:0 0 auto; height:2px; width:auto; transform:none; border-radius:0; }
`;
