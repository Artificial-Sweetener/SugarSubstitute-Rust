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
/** Define titlebar feedback and native field accent motion through Microsoft's Fluent tokens. */

/** Keep interaction styling scoped to positively identified native presentation boundaries. */
export function fluentInteractionCss(): string {
  return `
    [data-sc-fluent-node] [data-testid="node-collapse-button"][hidden] { display:none !important; }
    [data-sc-toggle-row], [data-sc-fluent-node] [data-testid="advanced-inputs-button"] {
      cursor:pointer; transition:background-color var(--durationFast) var(--curveEasyEase);
    }
    [data-sc-toggle-row]:hover, [data-sc-toggle-row]:focus-within,
    [data-sc-fluent-node] [data-testid="advanced-inputs-button"]:hover,
    [data-sc-fluent-node] [data-testid="advanced-inputs-button"]:focus-visible {
      background:var(--colorNeutralBackground1Hover);
    }
    [data-sc-toggle-row]:active, [data-sc-fluent-node] [data-testid="advanced-inputs-button"]:active {
      background:var(--colorNeutralBackground1Pressed);
    }
    [data-sc-toggle-row]:focus-visible { outline:2px solid var(--colorBrandStroke1); outline-offset:-2px; }
    [data-sc-toggle-state] { color:var(--colorNeutralForeground2); font-size:12px; white-space:nowrap; }
    [data-sc-titlebar-action] {
      position:relative; cursor:pointer;
      transition:background-color var(--durationFast) var(--curveEasyEase);
    }
    [data-sc-titlebar-action]:hover, [data-sc-titlebar-action]:focus-visible {
      background-color:var(--colorNeutralBackground1Hover);
    }
    [data-sc-titlebar-action]:active { background-color:var(--colorNeutralBackground1Pressed); }
    [data-sc-titlebar-action]:focus-visible { outline:2px solid var(--colorBrandStroke1); outline-offset:-2px; }
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
      rotate:0deg; transform:rotate(180deg); transition:transform var(--durationNormal) var(--curveDecelerateMid);
    }
    [data-sc-titlebar-collapsed="true"] [data-testid="node-collapse-button"] i { transform:rotate(0deg); }
    [data-sc-disclosure-target="collapsed"] [data-testid="node-collapse-button"] i { transform:rotate(0deg); }
    [data-sc-disclosure-target="expanded"] [data-testid="node-collapse-button"] i { transform:rotate(180deg); }
    [data-sc-fluent-edit-frame] { position:relative; }
    [data-sc-fluent-edit-frame]::after {
      content:''; position:absolute; left:0; right:0; bottom:0;
      height:2px; border-radius:0 0 4px 4px; pointer-events:none;
      background:var(--colorBrandStroke1); transform:scaleX(0); transform-origin:center;
      transition:transform var(--durationNormal) var(--curveDecelerateMid);
    }
    [data-sc-fluent-edit-frame]:has(input:focus,textarea:focus,[role="combobox"]:focus-visible,[role="combobox"][aria-expanded="true"])::after { transform:scaleX(1); }
    [data-sc-fluent-edit-frame]:has(input:focus,textarea:focus) {
      background-color:var(--colorNeutralBackground1);
    }
    [data-sc-fluent-edit-frame] input, [data-sc-fluent-edit-frame] textarea {
      caret-color:var(--colorBrandStroke1);
      transition:background-color var(--durationFast) var(--curveEasyEase), border-color var(--durationFast) var(--curveEasyEase);
    }
    [data-sc-fluent-edit-frame] textarea:focus {
      background:var(--colorNeutralBackground1); outline:none;
    }
    [data-sc-fluent-row][data-sc-fluent-widget^="WidgetInputNumber"] [data-sc-fluent-layout] > :last-child > :first-child {
      --tw-ring-shadow:0 0 transparent; --tw-ring-color:transparent;
    }
    @media (prefers-reduced-motion:reduce) {
      [data-sc-toggle-row], [data-sc-fluent-node] [data-testid="advanced-inputs-button"],
      [data-sc-titlebar-action], [data-sc-titlebar-action] [data-testid="node-collapse-button"] i,
      [data-sc-fluent-edit-frame]::after, [data-sc-fluent-edit-frame] input, [data-sc-fluent-edit-frame] textarea {
        transition:none;
      }
    }
  `;
}
