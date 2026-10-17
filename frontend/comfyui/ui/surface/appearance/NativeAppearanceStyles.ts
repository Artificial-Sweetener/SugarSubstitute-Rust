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
/** Share native card layout and control presentation across supported design systems. */

import { nativeInteractionCss } from './NativeInteractionStyles.js';
import { nativeNumericCss } from './NativeNumericStyles.js';
import { nativeSeedCss } from './NativeSeedStyles.js';
import { scrollbarCss } from './ScrollbarStyles.js';

/** Render the shared appearance without overriding explicit custom inline styling. */
export interface NativeAppearancePalette {
  readonly tokens: string;
  readonly canvas: string;
  readonly cardFill: string;
  readonly cardStroke: string;
  readonly rowStroke: string;
  readonly controlFill: string;
  readonly controlStroke: string;
  readonly controlBottomStroke: string;
  readonly radius: string;
}

/** Share host geometry and native value presentation independently of the selected design system. */
export function nativeAppearanceCss(palette: NativeAppearancePalette, dark: boolean): string {
  return `
    ${scrollbarCss(dark)}
    ${nativeInteractionCss()}
    [data-sc-style-theme-root] {
      ${palette.tokens}
      --sc-node-card-radius:${palette.radius};
      --sugarcubes-masonry-surface:${palette.canvas};
    }
    :where([data-sc-style-node], [data-sc-style-surface], [data-sc-style-popup]) {
      --sc-card-fill:${palette.cardFill};
      --sc-node-card-radius:${palette.radius};
      --sc-control-fill:${palette.controlFill};
      --sc-card-stroke:${palette.cardStroke};
      --sc-row-stroke:${palette.rowStroke};
      --sc-control-stroke:${palette.controlStroke};
      --sc-control-bottom-stroke:${palette.controlBottomStroke};
      --node-component-surface:var(--sc-card-fill);
      --node-component-header-surface:var(--sc-card-fill);
      --component-node-background:var(--sc-card-fill);
      --node-component-header:var(--sc-text);
      --node-component-slot-text:var(--sc-text-secondary);
      --node-component-widget-input:var(--sc-text);
      --node-component-widget-input-surface:var(--sc-surface);
      --node-component-surface-hovered:var(--sc-surface-hover);
      --component-node-widget-background:var(--sc-control-fill);
      --component-node-widget-background-hovered:var(--sc-surface-hover);
      --component-node-widget-background-highlighted:var(--sc-brand-border);
      --component-node-widget-background-disabled:var(--sc-surface-disabled);
      --component-node-foreground:var(--sc-text);
      --node-component-border:var(--sc-border-subtle);
      --node-stroke:var(--sc-border-subtle);
      --node-component-outline:var(--sc-brand-border);
      --interface-stroke:var(--sc-border-subtle);
      --p-inputtext-background:var(--sc-surface);
      --p-inputtext-color:var(--sc-text);
      --p-inputtext-border-color:var(--sc-border-subtle);
      --p-inputtext-focus-border-color:var(--sc-brand-border);
      --p-select-background:var(--sc-surface);
      --p-select-color:var(--sc-text);
      --p-select-border-color:var(--sc-border-subtle);
      --p-toggleswitch-checked-background:var(--sc-brand-indicator);
      --p-toggleswitch-checked-hover-background:var(--sc-brand-indicator-hover);
      --sugarcubes-cube-card-header:var(--sc-card-fill);
      --sugarcubes-cube-card-body:var(--sc-card-fill);
      --sugarcubes-cube-backdrop:var(--sugarcubes-masonry-surface);
      --sugarcubes-masonry-card-surface:var(--sc-card-fill);
      --sugarcubes-masonry-column-width:400;
      --sugarcubes-masonry-gap:8;
    }
    [data-sc-style-node], [data-sc-style-surface], [data-sc-style-popup] {
      color:var(--sc-text);
    }
    [data-sc-style-node] > [data-testid="node-inner-wrapper"],
    [data-sc-style-node] > [data-testid="node-state-outline-overlay"] {
      border-radius:var(--sc-node-card-radius);
    }
    [data-sc-style-node] > [data-testid="node-inner-wrapper"] {
      border:1px solid var(--sc-card-stroke);
    }
    [data-sc-style-node] > [data-testid="node-inner-wrapper"]::before {
      border-radius:inherit;
    }
    [data-sc-style-node] [data-testid^="node-header-"] {
      font-family:var(--sc-font-family); font-size:14px; font-weight:400;
      height:49px; min-height:49px; padding:0 10px;
      display:flex; align-items:center; border-radius:var(--sc-node-card-radius) var(--sc-node-card-radius) 0 0;
    }
    [data-sc-style-node][data-cube-face-body="header-only"] [data-testid^="node-header-"] {
      border-radius:var(--sc-node-card-radius);
    }
    [data-sc-style-node] [data-testid^="node-header-"][data-sc-titlebar-collapsed="true"] {
      border-radius:var(--sc-node-card-radius);
    }
    [data-sc-style-node] [data-testid^="node-header-"] > :first-child { width:100%; }
    [data-sc-style-node] [data-testid="node-title"] { font-size:14px; }
    .sugarcubes-cube-face__node-card [data-sc-style-node] [data-testid="node-title"] { padding-left:30px; }
    [data-sc-style-node] { filter:none; font-family:var(--sc-font-family); }
    [data-sc-style-node] [data-testid="node-title"] .editable-text { font-weight:400; padding-left:0; padding-right:0; }
    .sugarcubes-cube-face__node-card [data-sc-style-node] [data-testid^="node-body-"]:has(> [data-sc-style-fields]) {
      padding:0; gap:0;
    }
    [data-sc-style-node] [data-testid^="node-body-"]:not(:has(> :not([hidden]))) { padding:0; }
    [data-sc-style-node] [data-testid^="node-body-"] {
      border-radius:0 0 var(--sc-node-card-radius) var(--sc-node-card-radius);
    }
    [data-sc-style-node]:has([data-testid="advanced-inputs-button"]) [data-testid^="node-body-"] {
      border-radius:0;
    }
    [data-sc-style-node]:has([data-testid="advanced-inputs-button"]) [data-testid^="node-body-"]:has(> [data-sc-style-fields]) {
      padding-bottom:0;
    }
    [data-sc-style-node] [data-testid="advanced-inputs-button"] {
      border-radius:0 0 var(--sc-node-card-radius) var(--sc-node-card-radius); font-family:var(--sc-font-family);
      background:var(--sc-card-fill); border-top:1px solid var(--sc-row-stroke);
    }
    [data-sc-style-node]:has([data-sc-style-row]) [data-testid^="node-header-"]:not([data-sc-titlebar-collapsed="true"]) {
      border-bottom:1px solid var(--sc-row-stroke);
    }
    .sugarcubes-cube-face__node-card [data-sc-style-node]:has([data-sc-style-row]) [data-testid^="node-header-"]:not([data-sc-titlebar-collapsed="true"]) { border-bottom:0; }
    [data-sc-style-node] [data-sc-style-fields] { gap:0; }
    [data-sc-style-node] [data-testid^="node-body-"]:has(> [data-sc-style-ports]:not([hidden])) {
      padding-top:0; gap:0;
    }
    [data-sc-style-node] [data-sc-style-ports]:not([hidden]) {
      padding:8px 0; border-bottom:1px solid var(--sc-row-stroke);
    }
    [data-sc-style-node] [data-sc-style-row] { padding-top:8px; padding-bottom:8px; }
    .sugarcubes-cube-face__node-card [data-sc-style-node] [data-sc-style-row] { padding-left:10px; padding-right:30px; }
    .sugarcubes-cube-face__node-card [data-sc-style-row][data-sc-style-kind="multiline"],
    .sugarcubes-cube-face__node-card [data-sc-style-row][data-sc-style-span="1"][data-sc-style-column="1"] { padding-right:10px; }
    [data-sc-style-node] [data-sc-style-row]:not([hidden]):not([style*="display: none"]) ~ [data-sc-style-row]:not([hidden]):not([style*="display: none"]) {
      border-top:1px solid var(--sc-row-stroke);
    }
    .sugarcubes-cube-face__node-card [data-sc-style-fields] {
      display:grid; grid-template-columns:minmax(0,1fr) minmax(0,1fr);
      gap:0; padding:0;
    }
    .sugarcubes-cube-face__node-card [data-sc-style-fields] > :not([data-sc-style-row]) { grid-column:1 / -1; }
    .sugarcubes-cube-face__node-card [data-sc-style-row] {
      display:grid; grid-template-columns:20px minmax(0,1fr);
      grid-row:var(--sc-field-row); grid-column:var(--sc-field-column) / span var(--sc-field-span);
      min-width:0; border-top:1px solid var(--sc-row-stroke);
      font-family:var(--sc-font-family); font-size:12px; gap:0;
    }
    .sugarcubes-cube-face__node-card [data-sc-style-row][data-sc-style-column="2"] { position:relative; }
    .sugarcubes-cube-face__node-card [data-sc-style-row][data-sc-style-column="2"]::before {
      content:''; position:absolute; left:0; top:50%; transform:translateY(-50%);
      width:1px; height:33px; background:var(--sc-row-stroke); pointer-events:none;
    }
    .sugarcubes-cube-face__node-card [data-sc-style-row] > [data-sc-style-control], .sugarcubes-cube-face__node-card [data-sc-style-layout] {
      min-width:0; width:100%; height:auto; min-height:33px;
      grid-column:2;
    }
    .sugarcubes-cube-face__node-card [data-sc-style-layout] {
      display:flex; flex-direction:row; align-items:center; justify-content:space-between; gap:6px;
    }
    .sugarcubes-cube-face__node-card [data-sc-style-layout] > :first-child { min-width:0; flex:1; }
    .sugarcubes-cube-face__node-card [data-sc-style-layout]:not([data-sc-choice-inline] *) > :last-child { min-width:0; flex:0 1 156px; max-width:65%; }
    .sugarcubes-cube-face__node-card [data-sc-style-row][data-sc-style-widget^="WidgetInputNumber"] [data-sc-style-layout] > :last-child {
      flex-basis:100px;
    }
    .sugarcubes-cube-face__node-card [data-sc-style-row][data-sc-style-widget="WidgetToggleSwitch"] [data-sc-style-layout] > :last-child {
      flex:0 0 auto;
    }
    .sugarcubes-cube-face__node-card [data-sc-style-row] [data-sc-style-layout]:not([data-sc-choice-inline] *) > :last-child:has(.sc-model-mount) {
      flex-basis:360px; max-width:min(65%,360px);
    }
    .sugarcubes-cube-face__node-card [data-sc-style-row][data-sc-style-widget^="WidgetInputNumber"] [data-sc-style-layout] > :last-child > :first-child > :first-child {
      height:33px; border:var(--sc-field-border-width) solid var(--sc-control-stroke);
      border-bottom-color:var(--sc-control-bottom-stroke); border-radius:var(--sc-field-radius);
      background:var(--sc-control-fill);
    }
    [data-sc-style-combo-frame],
    :where([data-sc-style-node], [data-sc-style-surface]) .sc-model-field:not(:has(.sc-model-banner, .sc-model-transfer-thumbnail)) {
      box-sizing:border-box; height:33px; min-height:33px;
      border:var(--sc-field-border-width) solid var(--sc-control-stroke); border-bottom-color:var(--sc-control-bottom-stroke);
      border-radius:var(--sc-field-radius); background:var(--sc-control-fill);
    }
    [data-sc-style-combo-frame]:hover,
    :where([data-sc-style-node], [data-sc-style-surface]) .sc-model-field:not(:has(.sc-model-banner, .sc-model-transfer-thumbnail)):hover {
      background:var(--sc-surface-hover);
    }
    :where([data-sc-style-node], [data-sc-style-surface]) .sc-model-field:not(:has(.sc-model-banner, .sc-model-transfer-thumbnail)) .sc-model-field-arrow {
      color:var(--sc-text-muted);
    }
    [data-sc-style-combo-frame] > [data-testid="widget-select-default-trigger"] { height:100%; }
    .sugarcubes-cube-face__node-card [data-sc-style-row] [data-sc-style-control] input,
    .sugarcubes-cube-face__node-card [data-sc-style-row] [data-sc-style-control] textarea,
    .sugarcubes-cube-face__node-card [data-sc-style-row] [data-sc-style-control] button {
      font-family:var(--sc-font-family); font-size:12px;
    }
    .sugarcubes-cube-face__node-card [data-sc-style-row][data-sc-style-kind="multiline"] {
      display:block;
    }
    .sugarcubes-cube-face__node-card [data-sc-style-row][data-sc-style-kind="multiline"] > :first-child { display:none; }
    .sugarcubes-cube-face__node-card [data-sc-style-row][data-sc-style-kind="multiline"] [data-sc-style-control] textarea {
      border:var(--sc-field-border-width) solid var(--sc-control-stroke); border-bottom-color:var(--sc-control-bottom-stroke);
      border-radius:var(--sc-field-radius); background:var(--sc-control-fill);
      min-height:33px; padding:6px 8px; line-height:19px;
    }
    .sugarcubes-cube-face__node-card [data-sc-style-row][data-sc-style-kind="multiline"] [data-sc-style-control] > :first-child {
      display:none;
    }
    [data-sc-style-surface] { font-family:var(--sc-font-family); }
    .sugarcubes-cube-face__node-card:has([data-sc-style-node]) {
      border-radius:var(--sc-node-card-radius); border:0; background:transparent;
    }
    [data-sc-style-surface] .sugarcubes-workflow-masonry__section {
      border:0; color:var(--sc-text);
    }
    [data-sc-style-surface] .sugarcubes-workflow-masonry__section h2 { font-size:20px; font-weight:400; }
    [data-sc-style-surface].sugarcubes-workflow-masonry { --sc-masonry-inline-padding:6px; }
    [data-sc-style-node] .sugarcubes-cube-face__activation { background:transparent; }
    [data-sc-style-popup] {
      background:var(--sc-card-fill); color:var(--sc-text);
      border:1px solid var(--sc-border-subtle); border-radius:4px;
      box-shadow:var(--sc-shadow-medium); font-family:var(--sc-font-family); font-size:12px;
    }
    [data-sc-style-popup] [role="option"] { border-radius:4px; }
    [data-sc-style-popup] [role="option"][data-highlighted] {
      background:var(--sc-surface-hover);
    }
    [data-sc-style-popup]:has([data-sc-inline-search]) { overflow:visible; position:relative; }
    [data-sc-inline-search-shell] { height:0; min-height:0; padding:0; margin:0; border:0; }
    [data-sc-inline-search-shell] > :not(input) { display:none; }
    [data-sc-inline-search] {
      position:absolute; left:var(--sc-search-left); top:var(--sc-search-top);
      width:var(--sc-search-width); height:var(--sc-search-height);
      box-sizing:border-box; padding:0 8px;
      font-family:var(--sc-font-family); font-size:var(--sc-search-font);
      background:var(--sc-control-fill); color:var(--sc-text);
      border:var(--sc-field-border-width) solid var(--sc-control-stroke); border-bottom:2px solid var(--sc-brand-border);
      border-radius:4px; outline:none; z-index:1;
    }
    ${nativeNumericCss()}
    ${nativeSeedCss()}
  `;
}
