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

/** Scope the reference tab body, accent and cutout to workflow-specific Microsoft controls. */
export const workflowTabCss = `
  :host { height:30px; width:${String(WORKFLOW_TAB_WIDTH)}px; min-width:${String(WORKFLOW_TAB_WIDTH)}px; max-width:${String(WORKFLOW_TAB_WIDTH)}px; flex:0 0 ${String(WORKFLOW_TAB_WIDTH)}px; padding:0 8px 0 20px;
    border-radius:8px 8px 0 0; gap:6px; font:13px/20px var(--fontFamilyBase); overflow:hidden;
    color:var(--sc-frame-muted); background:transparent; box-shadow:none; }
  :host .tab-content { min-width:0; flex:1; overflow:hidden; padding:0; }
  :host .tab-content::after { display:none; }
  :host([aria-selected='true']) { background:var(--sc-frame-wash); color:var(--colorNeutralForeground1); font-weight:600; }
  :host(:not([aria-selected='true']):hover) { background:var(--sc-frame-hover); }
  :host(:not([aria-selected='true'])) { clip-path:inset(1px 1px 0 round 7px 7px 0 0); }
  :host(:not([aria-selected='true']):active) { background:var(--sc-frame-pressed); }
  :host([data-unread]:not([aria-selected='true'])) { background:var(--sc-frame-unread); }
  :host::after { content:''; position:absolute; inset:0 0 auto; height:2px; width:auto; border-radius:0;
    background:transparent; transform:none; transition:none; }
  :host([aria-selected='true'])::after { background:var(--sc-accent-indicator); }
  :host { --sc-tab-cutout-radius:0px; mask-image:radial-gradient(circle var(--sc-tab-cutout-radius) at var(--sc-tab-cutout-x,-17px) 25px,transparent calc(var(--sc-tab-cutout-radius) - 1px),#000 var(--sc-tab-cutout-radius)); }
  :host([data-orb-adjacent]) { padding-left:32px; --sc-tab-cutout-radius:25px; }
`;

/** Suppress Microsoft's bottom indicator while retaining its focus and selection machinery. */
export const workflowTablistCss = `
  :host::after { content:none !important; display:none !important; }
  :host([data-selected])::before { content:''; position:absolute; bottom:0;
    left:calc(var(--sc-selected-offset) - 8px); width:${String(WORKFLOW_TAB_WIDTH + 16)}px; height:4px;
    background:var(--sc-frame-wash); pointer-events:none; z-index:1;
    clip-path:path('M0 4H8V0C8 2.2 4.4 4 0 4ZM188 0V4H196C191.6 4 188 2.2 188 0Z');
    mask-image:radial-gradient(circle 25px at calc(-9px - var(--sc-selected-offset) + var(--sc-tab-scroll)) -1px,transparent 24px,#000 25px); }
  :host { height:30px; min-width:0; gap:0; flex:1; overflow-x:auto; overflow-y:hidden; scrollbar-width:none; }
  :host::-webkit-scrollbar { display:none; }
  :host ::slotted([slot='tab']) { padding:0 8px 0 20px; }
  :host ::slotted([slot='tab'][data-orb-adjacent]) { padding-left:32px; }
  :host ::slotted([slot='tab'])::before { content:none; }
  :host ::slotted([slot='tab'])::after { inset:0 0 auto; height:2px; width:auto; transform:none; border-radius:0; }
`;
