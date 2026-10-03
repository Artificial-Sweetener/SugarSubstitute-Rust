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
/** Define scoped Fluent tokens and SugarSubstitute card/row visual conventions. */
import { fluentTokenDeclarations } from './FluentThemeRuntime.js';
import { fluentInteractionCss } from './FluentInteractionStyles.js';
import { fluentNumericCss } from './FluentNumericStyles.js';
/** Render the shared appearance without overriding explicit custom inline styling. */
export function fluentAppearanceCss(dark) {
    return `
    ${fluentInteractionCss()}
    :where([data-sc-fluent-node], [data-sc-fluent-surface], [data-sc-fluent-popup]) {
      ${fluentTokenDeclarations(dark)}
      --sc-card-fill:${dark ? '#2b2b2b' : '#fbfbfb'};
      --sc-node-card-radius:4px;
      --sc-control-fill:${dark ? '#373737' : '#ffffff'};
      --sc-card-stroke:${dark ? 'rgba(0,0,0,.10)' : 'rgba(0,0,0,.06)'};
      --sc-row-stroke:var(--sc-card-stroke);
      --sc-control-stroke:${dark ? 'rgba(255,255,255,.07)' : 'rgba(0,0,0,.07)'};
      --sc-control-bottom-stroke:${dark ? 'rgba(255,255,255,.45)' : 'rgba(0,0,0,.45)'};
      --node-component-surface:var(--sc-card-fill);
      --node-component-header-surface:var(--sc-card-fill);
      --component-node-background:var(--sc-card-fill);
      --node-component-header:var(--colorNeutralForeground1);
      --node-component-slot-text:var(--colorNeutralForeground2);
      --node-component-widget-input:var(--colorNeutralForeground1);
      --node-component-widget-input-surface:var(--colorNeutralBackground1);
      --node-component-surface-hovered:var(--colorNeutralBackground1Hover);
      --component-node-widget-background:var(--sc-control-fill);
      --component-node-widget-background-hovered:var(--colorNeutralBackground1Hover);
      --component-node-widget-background-highlighted:var(--colorBrandStroke1);
      --component-node-widget-background-disabled:var(--colorNeutralBackgroundDisabled);
      --component-node-foreground:var(--colorNeutralForeground1);
      --node-component-border:var(--colorNeutralStroke2);
      --node-stroke:var(--colorNeutralStroke2);
      --node-component-outline:var(--colorBrandStroke1);
      --interface-stroke:var(--colorNeutralStroke2);
      --p-inputtext-background:var(--colorNeutralBackground1);
      --p-inputtext-color:var(--colorNeutralForeground1);
      --p-inputtext-border-color:var(--colorNeutralStroke2);
      --p-inputtext-focus-border-color:var(--colorBrandStroke1);
      --p-select-background:var(--colorNeutralBackground1);
      --p-select-color:var(--colorNeutralForeground1);
      --p-select-border-color:var(--colorNeutralStroke2);
      --p-toggleswitch-checked-background:var(--colorCompoundBrandBackground);
      --p-toggleswitch-checked-hover-background:var(--colorCompoundBrandBackgroundHover);
      --sugarcubes-cube-card-header:var(--sc-card-fill);
      --sugarcubes-cube-card-body:var(--sc-card-fill);
      --sugarcubes-masonry-surface:${dark ? '#202020' : '#f3f3f3'};
      --sugarcubes-masonry-card-surface:var(--sc-card-fill);
      --sugarcubes-masonry-column-width:400;
      --sugarcubes-masonry-gap:8;
    }
    [data-sc-fluent-node] > [data-testid="node-inner-wrapper"],
    [data-sc-fluent-node] > [data-testid="node-state-outline-overlay"] {
      border-radius:var(--sc-node-card-radius);
    }
    [data-sc-fluent-node] > [data-testid="node-inner-wrapper"] {
      border:1px solid var(--sc-card-stroke);
    }
    [data-sc-fluent-node] > [data-testid="node-inner-wrapper"]::before {
      border-radius:inherit;
    }
    [data-sc-fluent-node] [data-testid^="node-header-"] {
      font-family:var(--fontFamilyBase); font-size:14px; font-weight:400;
      height:49px; min-height:49px; padding:0 10px;
      display:flex; align-items:center; border-radius:var(--sc-node-card-radius) var(--sc-node-card-radius) 0 0;
    }
    [data-sc-fluent-node][data-cube-face-body="header-only"] [data-testid^="node-header-"] {
      border-radius:var(--sc-node-card-radius);
    }
    [data-sc-fluent-node] [data-testid^="node-header-"][data-sc-titlebar-collapsed="true"] {
      border-radius:var(--sc-node-card-radius);
    }
    [data-sc-fluent-node] [data-testid^="node-header-"] > :first-child { width:100%; }
    [data-sc-fluent-node] [data-testid="node-title"] { font-size:14px; }
    .sugarcubes-workflow-masonry [data-sc-fluent-node] [data-testid="node-title"] { padding-left:30px; }
    [data-sc-fluent-node] { filter:none; font-family:var(--fontFamilyBase); }
    [data-sc-fluent-node] [data-testid="node-title"] .editable-text { font-weight:400; padding-left:0; padding-right:0; }
    .sugarcubes-workflow-masonry [data-sc-fluent-node] [data-testid^="node-body-"]:has(> [data-sc-fluent-fields]) {
      padding:0; gap:0;
    }
    [data-sc-fluent-node] [data-testid^="node-body-"]:not(:has(> :not([hidden]))) { padding:0; }
    [data-sc-fluent-node] [data-testid^="node-body-"] {
      border-radius:0 0 var(--sc-node-card-radius) var(--sc-node-card-radius);
    }
    [data-sc-fluent-node]:has([data-testid="advanced-inputs-button"]) [data-testid^="node-body-"] {
      border-radius:0;
    }
    [data-sc-fluent-node]:has([data-testid="advanced-inputs-button"]) [data-testid^="node-body-"]:has(> [data-sc-fluent-fields]) {
      padding-bottom:0;
    }
    [data-sc-fluent-node] [data-testid="advanced-inputs-button"] {
      border-radius:0 0 var(--sc-node-card-radius) var(--sc-node-card-radius); font-family:var(--fontFamilyBase);
      background:var(--sc-card-fill); border-top:1px solid var(--sc-row-stroke);
    }
    [data-sc-fluent-node]:has([data-sc-fluent-row]) [data-testid^="node-header-"]:not([data-sc-titlebar-collapsed="true"]) {
      border-bottom:1px solid var(--sc-row-stroke);
    }
    .sugarcubes-workflow-masonry [data-sc-fluent-node]:has([data-sc-fluent-row]) [data-testid^="node-header-"]:not([data-sc-titlebar-collapsed="true"]) { border-bottom:0; }
    [data-sc-fluent-node] [data-sc-fluent-fields] { gap:0; }
    [data-sc-fluent-node] [data-testid^="node-body-"]:has(> [data-sc-fluent-ports]:not([hidden])) {
      padding-top:0; gap:0;
    }
    [data-sc-fluent-node] [data-sc-fluent-ports]:not([hidden]) {
      padding:8px 0; border-bottom:1px solid var(--sc-row-stroke);
    }
    [data-sc-fluent-node] [data-sc-fluent-row] { padding:8px 30px 8px 10px; }
    [data-sc-fluent-node] [data-sc-fluent-row][data-sc-fluent-kind="multiline"],
    .sugarcubes-workflow-masonry [data-sc-fluent-row][data-sc-fluent-span="1"][data-sc-fluent-column="1"] { padding-right:10px; }
    [data-sc-fluent-node] [data-sc-fluent-row]:not([hidden]):not([style*="display: none"]) ~ [data-sc-fluent-row]:not([hidden]):not([style*="display: none"]) {
      border-top:1px solid var(--sc-row-stroke);
    }
    .sugarcubes-workflow-masonry [data-sc-fluent-fields] {
      display:grid; grid-template-columns:minmax(0,1fr) minmax(0,1fr);
      gap:0; padding:0;
    }
    .sugarcubes-workflow-masonry [data-sc-fluent-fields] > :not([data-sc-fluent-row]) { grid-column:1 / -1; }
    .sugarcubes-workflow-masonry [data-sc-fluent-row] {
      display:grid; grid-template-columns:20px minmax(0,1fr);
      grid-row:var(--sc-field-row); grid-column:var(--sc-field-column) / span var(--sc-field-span);
      min-width:0; border-top:1px solid var(--sc-row-stroke);
      font-family:var(--fontFamilyBase); font-size:12px; gap:0;
    }
    .sugarcubes-workflow-masonry [data-sc-fluent-row][data-sc-fluent-column="2"] { position:relative; }
    .sugarcubes-workflow-masonry [data-sc-fluent-row][data-sc-fluent-column="2"]::before {
      content:''; position:absolute; left:0; top:50%; transform:translateY(-50%);
      width:1px; height:33px; background:var(--sc-row-stroke); pointer-events:none;
    }
    .sugarcubes-workflow-masonry [data-sc-fluent-row] > [data-sc-fluent-control], .sugarcubes-workflow-masonry [data-sc-fluent-layout] {
      min-width:0; width:100%; height:auto; min-height:33px;
      grid-column:2;
    }
    .sugarcubes-workflow-masonry [data-sc-fluent-layout] {
      display:flex; flex-direction:row; align-items:center; justify-content:space-between; gap:6px;
    }
    .sugarcubes-workflow-masonry [data-sc-fluent-layout] > :first-child { min-width:0; flex:1; }
    .sugarcubes-workflow-masonry [data-sc-fluent-layout] > :last-child { min-width:0; flex:0 1 156px; max-width:65%; }
    .sugarcubes-workflow-masonry [data-sc-fluent-row][data-sc-fluent-widget^="WidgetInputNumber"] [data-sc-fluent-layout] > :last-child {
      flex-basis:100px;
    }
    .sugarcubes-workflow-masonry [data-sc-fluent-row][data-sc-fluent-widget="WidgetToggleSwitch"] [data-sc-fluent-layout] > :last-child {
      flex:0 0 auto;
    }
    .sugarcubes-workflow-masonry [data-sc-fluent-row][data-sc-fluent-kind="model"] [data-sc-fluent-layout] > :last-child {
      flex-basis:360px; max-width:min(65%,360px);
    }
    .sugarcubes-workflow-masonry [data-sc-fluent-row][data-sc-fluent-widget^="WidgetInputNumber"] [data-sc-fluent-layout] > :last-child > :first-child > :first-child {
      height:33px; border:1px solid var(--sc-control-stroke);
      border-bottom-color:var(--sc-control-bottom-stroke); border-radius:4px;
      background:var(--sc-control-fill);
    }
    [data-sc-fluent-combo-frame] {
      box-sizing:border-box; height:33px; min-height:33px;
      border:1px solid var(--sc-control-stroke); border-bottom-color:var(--sc-control-bottom-stroke);
      border-radius:4px; background:var(--sc-control-fill);
    }
    [data-sc-fluent-combo-frame] > [data-testid="widget-select-default-trigger"] { height:100%; }
    .sugarcubes-workflow-masonry [data-sc-fluent-row] [data-sc-fluent-control] input,
    .sugarcubes-workflow-masonry [data-sc-fluent-row] [data-sc-fluent-control] textarea,
    .sugarcubes-workflow-masonry [data-sc-fluent-row] [data-sc-fluent-control] button {
      font-family:var(--fontFamilyBase); font-size:12px;
    }
    .sugarcubes-workflow-masonry [data-sc-fluent-row][data-sc-fluent-kind="multiline"] {
      display:block;
    }
    .sugarcubes-workflow-masonry [data-sc-fluent-row][data-sc-fluent-kind="multiline"] > :first-child { display:none; }
    .sugarcubes-workflow-masonry [data-sc-fluent-row][data-sc-fluent-kind="multiline"] [data-sc-fluent-control] textarea {
      border:1px solid var(--sc-control-stroke); border-bottom-color:var(--sc-control-bottom-stroke);
      border-radius:4px; background:var(--sc-control-fill);
      min-height:33px; padding:6px 8px; line-height:19px;
    }
    .sugarcubes-workflow-masonry [data-sc-fluent-row][data-sc-fluent-kind="multiline"] [data-sc-fluent-control] > :first-child {
      display:none;
    }
    [data-sc-fluent-surface] { font-family:var(--fontFamilyBase); }
    [data-sc-fluent-surface] .sugarcubes-workflow-masonry__card {
      border-radius:4px; border:0; background:transparent;
    }
    [data-sc-fluent-surface] .sugarcubes-workflow-masonry__section {
      border:0; color:var(--colorNeutralForeground1);
    }
    [data-sc-fluent-surface] .sugarcubes-workflow-masonry__section h2 { font-size:20px; font-weight:400; }
    [data-sc-fluent-surface].sugarcubes-workflow-masonry { padding-inline:6px; }
    [data-sc-fluent-node] .sugarcubes-cube-face__activation { background:transparent; }
    [data-sc-fluent-popup] {
      background:var(--sc-card-fill); color:var(--colorNeutralForeground1);
      border:1px solid var(--colorNeutralStroke2); border-radius:4px;
      box-shadow:var(--shadow16); font-family:var(--fontFamilyBase); font-size:12px;
    }
    [data-sc-fluent-popup] [role="option"] { border-radius:4px; }
    [data-sc-fluent-popup] [role="option"][data-highlighted] {
      background:var(--colorNeutralBackground1Hover);
    }
    [data-sc-fluent-popup]:has([data-sc-inline-search]) { overflow:visible; position:relative; }
    [data-sc-inline-search-shell] { height:0; min-height:0; padding:0; margin:0; border:0; }
    [data-sc-inline-search-shell] > :not(input) { display:none; }
    [data-sc-inline-search] {
      position:absolute; left:var(--sc-search-left); top:var(--sc-search-top);
      width:var(--sc-search-width); height:var(--sc-search-height);
      box-sizing:border-box; padding:0 8px;
      font-family:var(--fontFamilyBase); font-size:var(--sc-search-font);
      background:var(--sc-control-fill); color:var(--colorNeutralForeground1);
      border:1px solid var(--sc-control-stroke); border-bottom:2px solid var(--colorBrandStroke1);
      border-radius:4px; outline:none; z-index:1;
    }
    ${fluentNumericCss()}
  `;
}
