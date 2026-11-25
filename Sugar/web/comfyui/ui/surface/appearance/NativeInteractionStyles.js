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
/** Define titlebar feedback and native field accent motion through scoped design-system tokens. */
import { fieldUnderlineCss } from './FieldSurfaceStyles.js';
/** Keep interaction styling scoped to positively identified native presentation boundaries. */
export function nativeInteractionCss() {
    return `
    [data-sc-disclosure-layout="fill"][data-sc-disclosing] > .lg-node[data-sc-style-node] {
      height:100% !important; min-height:0 !important;
    }
    [data-sc-disclosure-layout="fill"][data-sc-disclosing] [data-testid="node-inner-wrapper"],
    [data-sc-disclosure-layout="fill"][data-sc-disclosing] [data-testid^="node-body-"] {
      min-height:0;
    }
    [data-sc-disclosure-layout="fill"][data-sc-disclosing] [data-sc-style-fields] {
      min-height:0; flex:1; overflow:clip;
    }
    [data-sc-disclosure-layout="fill"][data-sc-disclosing] [data-testid^="node-header-"],
    [data-sc-disclosure-layout="fill"][data-sc-disclosing] > .lg-node > :has(> [data-testid="advanced-inputs-button"]) {
      flex-shrink:0;
    }
    [data-sc-boolean-native] { display:none !important; }
    [data-sc-boolean-segments] {
      display:flex; min-width:0; align-items:stretch; anchor-scope:--tab;
      border:1px solid var(--sc-border-subtle); border-radius:var(--sc-control-radius);
      background:var(--sc-surface-tertiary);
    }
    [data-sc-boolean-segments]::after {
      transition-duration:var(--sc-motion-duration);
      transition-timing-function:var(--sc-motion-ease);
    }
    [data-sc-boolean-segment] {
      flex:1 1 auto; min-width:0; overflow:hidden; white-space:nowrap;
    }

    [data-sc-boolean-checkbox] { flex:none; }
    [data-sc-boolean-control] {
      display:inline-flex; flex-direction:row; align-items:center; gap:8px;
      cursor:pointer; color:var(--sc-text); white-space:nowrap;
    }
    [data-sc-boolean-control]:has([data-sc-boolean-checkbox][disabled]) {
      cursor:default; color:var(--sc-text-disabled);
    }
    [data-sc-style-node] [data-testid="node-collapse-button"][hidden] { display:none !important; }
    [data-sc-toggle-row], [data-sc-style-node] [data-testid="advanced-inputs-button"] {
      cursor:pointer; transition:background-color var(--sc-motion-fast) var(--sc-motion-ease);
    }
    [data-sc-toggle-row]:hover,
    [data-sc-style-node] [data-testid="advanced-inputs-button"]:hover,
    [data-sc-style-node] [data-testid="advanced-inputs-button"]:focus-visible {
      background:var(--sc-surface-hover);
    }
    [data-sc-toggle-row]:active, [data-sc-style-node] [data-testid="advanced-inputs-button"]:active {
      background:var(--sc-surface-pressed);
    }
    [data-sc-toggle-row]:focus-visible { outline:2px solid var(--sc-brand-border); outline-offset:-2px; }
    [data-sc-titlebar-action] {
      position:relative; cursor:pointer;
      transition:background-color var(--sc-motion-fast) var(--sc-motion-ease);
    }
    [data-sc-titlebar-action]:hover, [data-sc-titlebar-action]:focus-visible {
      background-color:var(--sc-surface-hover);
    }
    [data-sc-titlebar-action]:active { background-color:var(--sc-surface-pressed); }
    [data-sc-titlebar-action]:focus-visible { outline:2px solid var(--sc-brand-border); outline-offset:-2px; }
    [data-sc-titlebar-controls] { position:relative; }
    [data-sc-titlebar-controls] > :first-child { padding-right:0; }
    [data-sc-titlebar-controls]:has([data-testid="node-collapse-button"]:not([hidden])) > :first-child {
      padding-right:20px;
    }
    [data-sc-titlebar-controls] [data-cube-card-activation] { padding-left:0; padding-right:8px; }
    [data-sc-titlebar-controls] > :first-child > :first-child { position:static; }
    [data-sc-titlebar-controls] > :first-child > :first-child > :has(> [data-testid="node-collapse-button"]) { display:contents; }
    [data-sc-titlebar-controls] [data-testid="node-collapse-button"] {
      position:absolute; right:5px; top:50%; transform:translateY(-50%); width:24px; height:24px;
    }
    [data-sc-titlebar-controls] [data-testid="node-collapse-button"] i {
      width:14px; height:14px; line-height:14px;
      rotate:0deg; transform:rotate(180deg); transition:transform var(--sc-motion-duration) var(--sc-motion-decelerate);
    }
    [data-sc-titlebar-collapsed="true"] [data-testid="node-collapse-button"] i { transform:rotate(0deg); }
    [data-sc-disclosure-target="collapsed"] [data-testid="node-collapse-button"] i { transform:rotate(0deg); }
    [data-sc-disclosure-target="expanded"] [data-testid="node-collapse-button"] i { transform:rotate(180deg); }
    /* A field's frame and the text box in it share the control radius, so the focus underline,
       cut by the frame's corners, ends exactly where the box's edge curves. ComfyUI's own focus
       ring around a text box is not drawn: the underline is the field's focus mark, as on
       numbers and choices. */
    [data-sc-style-edit-frame] { position:relative; border-radius:var(--sc-control-radius,4px); }
    [data-sc-style-edit-frame] > :is(input,textarea) { border-radius:inherit; }
    :has(> [data-sc-style-edit-frame] > :is(input,textarea)) { --tw-ring-shadow:0 0 transparent; --tw-ring-color:transparent; }
    ${fieldUnderlineCss('[data-sc-style-edit-frame]', '[data-sc-style-edit-frame]:has(input:focus,textarea:focus,[role="combobox"]:focus-visible,[role="combobox"][aria-expanded="true"])')}
    [data-sc-style-edit-frame] input, [data-sc-style-edit-frame] textarea {
      caret-color:var(--sc-brand-border);
    }
    [data-sc-style-edit-frame] textarea:focus { outline:none; }
    [data-sc-style-edit-frame]:has([data-sc-editor-field="fluent"], [data-sc-editor-field="spectrum"], [data-sc-editor-field="gimp"]) {
      --tw-ring-shadow:0 0 transparent; --tw-ring-color:transparent;
      background:none; border:none; outline:none; box-shadow:none;
    }
    [data-sc-style-row][data-sc-style-widget^="WidgetInputNumber"] [data-sc-style-layout] > :last-child > :first-child,
    [data-sc-style-row][data-sc-style-widget="WidgetSelect"] [data-sc-style-layout] > :last-child > :first-child,
    [data-sc-style-layout] > :last-child > :first-child:has(> [data-sc-boolean-segments]) {
      --tw-ring-shadow:0 0 transparent; --tw-ring-color:transparent;
    }
    @media (prefers-reduced-motion:reduce) {
      [data-sc-boolean-segments]::after,
      [data-sc-toggle-row], [data-sc-style-node] [data-testid="advanced-inputs-button"],
      [data-sc-titlebar-action], [data-sc-titlebar-action] [data-testid="node-collapse-button"] i {
        transition:none;
      }
    }
  `;
}
