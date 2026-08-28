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
/** Install structural Cube-face styles without reproducing Comfy node styling. */
const STYLE_ID = 'sugarcubes-cube-surface-styles';
/** Install the one structural stylesheet used by every Cube surface. */
export function ensureCubeSurfaceStyles(documentRef) {
    if (documentRef.getElementById(STYLE_ID))
        return;
    const style = documentRef.createElement('style');
    style.id = STYLE_ID;
    style.textContent = `
    .lg-node[data-sugarcube-node="true"] {
      overflow: visible;
    }
    .lg-node[data-sugarcube-node="true"] [data-sugarcube-native-hidden] {
      display: none !important;
    }
    .lg-node[data-sugarcube-node="true"] > [aria-label^="Resize from "] {
      z-index: 1002;
    }
    .sugarcubes-cube-node-edge-resize {
      position: absolute;
      z-index: 1000;
      pointer-events: auto;
      background: transparent;
    }
    .sugarcubes-cube-node-edge-resize--n,
    .sugarcubes-cube-node-edge-resize--s {
      right: 1.25rem;
      left: 1.25rem;
      height: 0.625rem;
      cursor: ns-resize;
    }
    .sugarcubes-cube-node-edge-resize--n {
      top: -0.3125rem;
    }
    .sugarcubes-cube-node-edge-resize--s {
      bottom: -0.3125rem;
    }
    .sugarcubes-cube-node-edge-resize--e,
    .sugarcubes-cube-node-edge-resize--w {
      top: 1.25rem;
      bottom: 1.25rem;
      width: 0.625rem;
      cursor: ew-resize;
    }
    .sugarcubes-cube-node-edge-resize--e {
      right: -0.3125rem;
    }
    .sugarcubes-cube-node-edge-resize--w {
      left: -0.3125rem;
    }
    .sugarcubes-cube-node-face-host {
      box-sizing: border-box;
      display: flex;
      flex: 1 1 auto;
      width: 100%;
      min-width: 0;
      min-height: 0;
      overflow: clip;
      contain: layout paint style;
      pointer-events: auto;
    }
    /* Comfy's select uses an in-tree fixed Reka popper. Let that active popup
       leave the Cube's normal paint-containment boundary without making every
       Cube face an uncontained paint surface. */
    .lg-node[data-sugarcube-node="true"]:has([data-reka-popper-content-wrapper]) {
      z-index: 10000 !important;
    }
    .sugarcubes-cube-node-face-host:has([data-reka-popper-content-wrapper]) {
      overflow: visible;
      contain: layout style;
    }
    [data-sugarcube-cube-body] {
      position: relative;
      padding-top: 0 !important;
      padding-bottom: 0 !important;
      overflow: visible !important;
      background-color: var(
        --sugarcubes-cube-backdrop,
        var(--component-node-background)
      ) !important;
    }
    .lg-node[data-sugarcube-node="true"]
      [data-sugarcube-boundary-row] {
      position: absolute;
      z-index: 1001;
      inset: 0;
      display: block;
      pointer-events: none;
    }
    .lg-node[data-sugarcube-node="true"]
      [data-sugarcube-boundary-row] > * {
      display: contents;
    }
    .lg-node[data-sugarcube-node="true"]
      [data-sugarcube-boundary-direction] {
      position: absolute;
      top: var(--sugarcube-boundary-position);
      z-index: 1003;
      width: max-content;
      min-width: 2rem;
      min-height: 2rem;
      overflow: visible;
      pointer-events: auto;
      transform: translateY(-50%);
    }
    .lg-node[data-sugarcube-node="true"]
      [data-sugarcube-boundary-direction="input"] {
      left: 0;
      box-sizing: border-box;
      width: 5.25rem;
      max-width: 5.25rem;
      padding-right: 0.5rem;
      overflow: visible;
    }
    .lg-node[data-sugarcube-node="true"]
      [data-sugarcube-boundary-direction="input"]
      > :not([data-testid="slot-connection-dot"]) {
      min-width: 0;
      max-width: 3.75rem;
      overflow: hidden;
      text-overflow: ellipsis;
      white-space: nowrap;
    }
    .lg-node[data-sugarcube-node="true"]
      [data-sugarcube-boundary-direction="output"] {
      right: 0;
      width: 2.5rem;
      min-width: 2.5rem;
      padding-left: 0 !important;
    }
    .lg-node[data-sugarcube-node="true"]
      [data-sugarcube-boundary-direction="output"]
      > :not([data-testid="slot-connection-dot"]) {
      display: none !important;
    }
    .sugarcubes-cube-port-leaders {
      position: absolute;
      z-index: 1002;
      inset: 0;
      display: block;
      width: 100%;
      height: 100%;
      overflow: hidden;
      pointer-events: none;
    }
    .sugarcubes-cube-face {
      box-sizing: border-box;
      position: relative;
      display: flex;
      flex-direction: column;
      flex: 1 1 0;
      width: 100%;
      height: auto;
      min-width: 0;
      min-height: 0;
      overflow: clip;
      color: inherit;
    }
    .sugarcubes-cube-face:has([data-reka-popper-content-wrapper]) {
      overflow: visible;
    }
    .sugarcubes-cube-face__header {
      position: relative;
      z-index: 4;
      display: grid;
      grid-template-columns: minmax(0, 1fr) auto minmax(0, 1fr);
      align-items: center;
      gap: 0.5rem;
      flex: 0 0 auto;
      padding: 0.25rem 0.5rem 0.5rem;
      background: var(--comfy-menu-bg, #171b20);
      border-bottom: 1px solid color-mix(in srgb, currentColor 14%, transparent);
      user-select: none;
    }
    .lg-node[data-sugarcube-node="true"]
      [data-testid^="node-header-"]
      > .sugarcubes-cube-face__header {
      box-sizing: border-box;
      width: 100%;
      padding: 0;
      border-bottom: 0;
      background: transparent;
    }
    .sugarcubes-cube-face__identity {
      display: flex;
      align-items: center;
      min-width: 0;
      gap: 0.5rem;
      justify-self: start;
    }
    .sugarcubes-cube-face__icon {
      display: inline-flex;
      align-items: center;
      justify-content: center;
      width: 1.5rem;
      height: 1.5rem;
      flex: 0 0 1.5rem;
      overflow: hidden;
      color: #fff;
    }
    .sugarcubes-cube-face__icon img,
    .sugarcubes-cube-face__icon canvas {
      display: block;
      width: 100%;
      height: 100%;
      object-fit: contain;
    }
    .sugarcubes-cube-face__title {
      min-width: 0;
      overflow: hidden;
      text-overflow: ellipsis;
      white-space: nowrap;
      font-size: 1rem;
    }
    .sugarcubes-cube-face__definition-badge {
      display: flex;
      flex-direction: column;
      align-items: center;
      grid-column: 2;
      width: 100%;
      min-width: 0;
      overflow: hidden;
      color: color-mix(in srgb, currentColor 86%, transparent);
      font-size: 0.75rem;
      line-height: 1.1;
      text-align: center;
    }
    .sugarcubes-cube-face__definition-name,
    .sugarcubes-cube-face__definition-source {
      display: block;
      max-width: 100%;
      overflow: hidden;
      text-overflow: ellipsis;
      white-space: nowrap;
    }
    .sugarcubes-cube-face__definition-source {
      color: color-mix(in srgb, currentColor 66%, transparent);
      font-size: 0.625rem;
    }
    .sugarcubes-cube-face__actions {
      position: relative;
      display: flex;
      grid-column: 3;
      gap: 0.375rem;
      justify-self: end;
    }
    .sugarcubes-cube-face__actions button {
      appearance: none;
      display: inline-grid;
      place-items: center;
      width: 2rem;
      height: 2rem;
      border: 0;
      background: transparent;
      color: inherit;
      padding: 0;
      font: inherit;
      font-size: 1rem;
    }
    .sugarcubes-cube-face__actions button[hidden] {
      display: none;
    }
    .sugarcubes-cube-face__actions .pi {
      font-size: 1rem;
      line-height: 1;
    }
    .sugarcubes-cube-unsaved-indicator {
      position: relative;
      display: inline-grid;
      flex: 0 0 auto;
      place-items: center;
      width: 1.25rem;
      height: 1.25rem;
      color: inherit;
    }
    .sugarcubes-cube-unsaved-indicator .pi {
      position: absolute;
      inset: auto;
      line-height: 1;
    }
    .sugarcubes-cube-unsaved-indicator__save.pi {
      font-size: 0.875rem;
      opacity: 0.9;
    }
    .sugarcubes-cube-unsaved-indicator__ban.pi {
      color: var(--p-orange-400, #fb923c);
      font-size: 1.1875rem;
      filter: drop-shadow(0 0 1px var(--comfy-menu-bg, #171b20));
    }
    .sugarcubes-cube-face__actions,
    .sugarcubes-cube-face__content {
      cursor: default;
      user-select: auto;
    }
    .sugarcubes-cube-face__actions button { cursor: pointer; }
    .sugarcubes-cube-face__content {
      box-sizing: border-box;
      position: relative;
      z-index: 1;
      display: flex;
      flex: 1 1 auto;
      min-width: 0;
      min-height: 0;
      gap: 0.5rem;
      overflow: clip;
      isolation: isolate;
      width: calc(
        100% -
        var(--sugarcubes-cube-input-gutter-width, 0px) -
        var(--sugarcubes-cube-output-gutter-width, 0px)
      );
      margin-right: var(--sugarcubes-cube-output-gutter-width, 0px);
      margin-left: var(--sugarcubes-cube-input-gutter-width, 0px);
      padding:
        var(--sugarcubes-cube-masonry-header-inset, 0px)
        0.5rem
        var(--sugarcubes-cube-masonry-footer-inset, 0px);
    }
    .sugarcubes-cube-face__content:has([data-reka-popper-content-wrapper]) {
      overflow: visible;
      isolation: auto;
    }
    .sugarcubes-cube-face__masonry {
      flex: 0 0 auto;
      min-width: 0;
    }
    .sugarcubes-cube-face__node-card {
      position: relative;
    }
    .sugarcubes-cube-face__native-card-mount {
      width: 100%;
      min-width: 0;
    }
    .sugarcubes-cube-face__node-card:focus-within {
      z-index: 10;
    }
    .sugarcubes-native-node-card
      [data-testid^="node-header-"][data-sugarcube-card-header-accessory] {
      overflow: clip;
    }
    .sugarcubes-cube-face__activation {
      position: static;
      z-index: auto;
      display: inline-flex;
      flex: 0 0 auto;
      align-items: center;
      align-self: center;
      gap: 0.45rem;
      min-width: 0;
      max-height: 100%;
      margin-inline-start: auto;
      padding: 0 0.25rem;
      border-radius: 999px;
      background: transparent;
      color: inherit;
      cursor: pointer;
      font-size: 0.75rem;
      line-height: 1;
    }
    .sugarcubes-cube-face__activation-label {
      color: color-mix(in srgb, currentColor 82%, transparent);
      white-space: nowrap;
    }
    .sugarcubes-cube-face__activation .p-toggleswitch {
      flex: 0 0 auto;
    }
    .sugarcubes-native-node-card {
      box-sizing: border-box;
      min-width: 0;
    }
    .sugarcubes-native-node-card .lg-node[data-cube-face-presentation="true"] {
      position: relative !important;
      transform: none !important;
      inset: auto !important;
      z-index: 0 !important;
      width: 100% !important;
      height: auto !important;
      --node-width: 100% !important;
      --node-height: auto !important;
      --min-node-width: 0px !important;
    }
    .sugarcubes-native-node-card .lg-node,
    .sugarcubes-native-node-card [data-testid^="node-inner-wrapper"] {
      min-width: 0 !important;
      max-width: 100%;
    }
    .sugarcubes-native-node-card [data-testid="node-inner-wrapper"] {
      background-color: var(--sugarcubes-cube-card-header) !important;
    }
    .sugarcubes-native-node-card [data-testid^="node-body-"] {
      background-color: var(--sugarcubes-cube-card-body) !important;
    }
    .sugarcubes-native-node-card
      [data-cube-face-body="header-only"]
      [data-testid="node-inner-wrapper"] {
      overflow: hidden;
    }
    .sugarcubes-native-node-card .lg-node-slots,
    .sugarcubes-native-node-card .lg-slot,
    .sugarcubes-native-node-card [data-testid="slot-connection-dot"],
    .sugarcubes-native-node-card [data-testid="subgraph-enter-button"] {
      display: none !important;
    }
    .sugarcubes-cube-face__preview {
      box-sizing: border-box;
      position: relative;
      align-self: stretch;
      flex: 0 0 auto;
      height: auto;
      min-height: 0;
      overflow: hidden;
      border-left: 1px solid color-mix(in srgb, currentColor 20%, transparent);
      --sugarcubes-cube-preview-edge-inset: 1.125rem;
    }
    .sugarcubes-cube-face__preview-divider {
      appearance: none;
      position: relative;
      z-index: 3;
      display: block;
      align-self: stretch;
      flex: 0 0 0.5rem;
      min-width: 0.5rem;
      margin: 0;
      padding: 0;
      border: 0;
      background: transparent;
      color: inherit;
      cursor: col-resize;
      touch-action: none;
    }
    .sugarcubes-cube-face__preview-divider[hidden] {
      display: none;
    }
    .sugarcubes-cube-face__preview-divider::after {
      content: '';
      position: absolute;
      top: 0;
      right: 0;
      bottom: 0;
      width: 1px;
      background: color-mix(in srgb, currentColor 20%, transparent);
      transition: background-color 120ms ease;
    }
    .sugarcubes-cube-face__preview-divider:hover::after,
    .sugarcubes-cube-face__preview-divider:focus-visible::after {
      background: color-mix(in srgb, currentColor 62%, transparent);
    }
    .sugarcubes-cube-face__content[data-preview-layout="stacked"] {
      gap: 0.75rem;
    }
    .sugarcubes-cube-face__content[data-preview-layout="stacked"]
      .sugarcubes-cube-face__preview {
      border-top: 1px solid color-mix(in srgb, currentColor 20%, transparent);
      border-left: 0;
      padding-top: 0.75rem;
      --sugarcubes-cube-preview-edge-inset: 0;
    }
    .sugarcubes-cube-face__preview-media {
      position: absolute;
      inset: 0 var(--sugarcubes-cube-preview-edge-inset);
      display: flex;
      flex-direction: column;
      gap: 0.75rem;
      height: 100%;
      min-height: 0;
      overflow: hidden;
    }
    .sugarcubes-cube-face__preview-outputs {
      display: grid;
      grid-template-columns: minmax(0, 1fr);
      align-items: stretch;
      gap: 0.75rem;
      min-width: 0;
      min-height: 0;
      height: 100%;
      overflow: hidden;
      flex: 1 1 0;
    }
    .sugarcubes-cube-face__preview-output {
      display: flex;
      flex-direction: column;
      gap: var(--sugarcubes-cube-preview-row-gap, 0.75rem);
      min-width: 0;
      min-height: 0;
      overflow: hidden;
    }
    .sugarcubes-cube-face__preview-output {
      height: 100%;
    }
    .sugarcubes-cube-face__preview-output + .sugarcubes-cube-face__preview-output {
      border-top: 1px solid color-mix(in srgb, currentColor 20%, transparent);
      padding-top: 0.75rem;
    }
    .sugarcubes-cube-face__preview-output-title {
      flex: 0 0 auto;
      min-width: 0;
    }
    .sugarcubes-cube-face__preview-output-title [data-cube-preview-output-label] {
      display: block;
      width: fit-content;
      max-width: 100%;
      overflow: hidden;
      text-overflow: ellipsis;
      white-space: nowrap;
    }
    .sugarcubes-cube-face__preview-items {
      display: grid;
      flex: 1 1 0;
      min-width: 0;
      min-height: 0;
      overflow: hidden;
    }
    .sugarcubes-cube-face__preview-output p {
      min-height: 0;
      margin: 0;
      overflow: hidden;
    }
    .sugarcubes-cube-face__preview-media figure {
      position: relative;
      display: flex;
      width: 100%;
      height: 100%;
      min-height: 0;
      overflow: hidden;
      margin: 0;
    }
    .sugarcubes-cube-face__preview-download {
      appearance: none;
      position: absolute;
      top: 0.5rem;
      right: 0.5rem;
      z-index: 2;
      display: inline-grid;
      place-items: center;
      width: 2rem;
      height: 2rem;
      padding: 0;
      border: 0;
      border-radius: 0.5rem;
      background: var(--fg-color, #f2f2f2);
      box-shadow: 0 0.125rem 0.5rem rgb(0 0 0 / 35%);
      color: var(--bg-color, #151515);
      cursor: pointer;
      opacity: 0;
      pointer-events: none;
      transition: opacity 120ms ease;
    }
    .sugarcubes-cube-face__preview-media figure:hover
      .sugarcubes-cube-face__preview-download,
    .sugarcubes-cube-face__preview-media figure:focus-within
      .sugarcubes-cube-face__preview-download {
      opacity: 1;
      pointer-events: auto;
    }
    .sugarcubes-cube-face__preview-media img {
      display: block;
      width: 100%;
      height: 100% !important;
      min-height: 0;
      max-height: 100% !important;
      object-fit: contain;
      object-position: center top;
      position: relative;
      z-index: 1;
      opacity: 1 !important;
      visibility: visible !important;
      border-radius: 0;
    }
    body.sugarcubes-cube-editor-workspace
      .p-splitter:has(.sugarcubes-sidebar-panel)
      > .side-bar-panel,
    body.sugarcubes-cube-editor-workspace
      .p-splitter:has(.sugarcubes-sidebar-panel)
      > .p-splitter-gutter {
      display: none !important;
    }
    .sugarcubes-cube-editor-metadata {
      box-sizing: border-box;
      position: fixed;
      top: var(--sugarcubes-cube-editor-metadata-top, 7.5rem);
      left: var(--sugarcubes-cube-editor-metadata-left, 1rem);
      z-index: 1000;
      width: min(
        21rem,
        calc(100vw - var(--sugarcubes-cube-editor-metadata-left, 1rem) - 1rem)
      );
      overflow: hidden;
      border: 1px solid var(--border-color, color-mix(in srgb, var(--fg-color, #ddd) 20%, transparent));
      border-radius: var(--p-border-radius-md, 0.5rem);
      background: var(--comfy-menu-bg, var(--p-content-background, #171b20));
      box-shadow: var(--shadow-lg, 0 2px 12px rgb(0 0 0 / 28%));
      color: var(--fg-color, var(--p-text-color, #ddd));
      pointer-events: auto;
    }
    .dark-theme .sugarcubes-cube-editor-metadata {
      box-shadow: var(--shadow-xl, var(--shadow-lg, 0 4px 20px rgb(0 0 0 / 45%)));
    }
    .sugarcubes-cube-editor-metadata__titlebar {
      display: flex;
      align-items: center;
      gap: 0.5rem;
      min-height: 2.25rem;
      padding: 0 0.5rem 0 0.75rem;
      border-bottom: 1px solid var(--border-color, color-mix(in srgb, var(--fg-color, #ddd) 15%, transparent));
      background: var(--comfy-menu-bg, var(--p-content-background, #171b20));
    }
    .sugarcubes-cube-editor-metadata.is-collapsed .sugarcubes-cube-editor-metadata__titlebar {
      border-bottom: 0;
    }
    .sugarcubes-cube-editor-metadata__titlebar strong {
      min-width: 0;
      overflow: hidden;
      text-overflow: ellipsis;
      white-space: nowrap;
      font-size: 0.875rem;
    }
    .sugarcubes-cube-editor-metadata__save,
    .sugarcubes-cube-editor-metadata__collapse {
      flex: 0 0 auto;
      border: 0;
      border-radius: var(--p-border-radius-sm, 0.25rem);
      background: transparent;
      color: inherit;
      display: inline-grid;
      place-items: center;
      width: 2rem;
      height: 2rem;
      padding: 0;
      cursor: pointer;
      font: inherit;
      font-size: 0.75rem;
    }
    .sugarcubes-cube-editor-metadata__save {
      margin-left: auto;
      background: var(--p-primary-color, #b58cff);
      color: var(--p-primary-contrast-color, #111);
    }
    .sugarcubes-cube-editor-metadata__save:disabled {
      cursor: default;
      opacity: 0.55;
    }
    .sugarcubes-cube-editor-metadata__collapse:hover,
    .sugarcubes-cube-editor-metadata__collapse:focus-visible {
      background: var(--p-content-hover-background, rgb(255 255 255 / 10%));
    }
    .sugarcubes-cube-editor-metadata__save .pi,
    .sugarcubes-cube-editor-metadata__collapse .pi {
      font-size: 0.875rem;
      line-height: 1;
    }
    .sugarcubes-cube-editor-metadata__body {
      display: grid;
      gap: 0.625rem;
      padding: 0.75rem;
    }
    .sugarcubes-cube-editor-metadata__field {
      display: grid;
      gap: 0.25rem;
      min-width: 0;
      color: var(--fg-color, var(--p-text-color, #ddd));
      font-size: 0.8125rem;
    }
    .sugarcubes-cube-editor-metadata__field > span {
      color: var(--p-text-muted-color, var(--fg-color, #aaa));
      font-size: 0.75rem;
    }
    .sugarcubes-cube-editor-metadata__field > strong {
      overflow-wrap: anywhere;
      font-weight: 500;
    }
    .sugarcubes-cube-editor-metadata__field .p-inputtext {
      box-sizing: border-box;
      width: 100%;
      border: 1px solid var(--p-inputtext-border-color, var(--border-color, #444));
      border-radius: var(--p-border-radius-sm, 0.25rem);
      background: var(--p-inputtext-background, var(--comfy-menu-bg, #171b20));
      color: var(--p-inputtext-color, var(--fg-color, #ddd));
      padding: 0.4rem 0.5rem;
      font: inherit;
    }
    .sugarcubes-cube-editor-metadata__target-model,
    .sugarcubes-cube-editor-metadata__model-support {
      display: grid;
      gap: 0.375rem;
      min-width: 0;
    }
    .sugarcubes-cube-editor-metadata .sugarcubes-comfy-settings-select-host,
    .sugarcubes-cube-editor-metadata .sugarcubes-comfy-settings-autocomplete-host {
      min-width: 0;
    }
    .sugarcubes-cube-editor-metadata__target-model-combo {
      min-width: 0;
    }
    .sugarcubes-cube-editor-metadata__field textarea.p-inputtext {
      resize: vertical;
    }
    .sugarcubes-cube-editor-metadata__error {
      margin: 0;
      border-left: 0.2rem solid var(--p-red-400, #f87171);
      color: var(--p-red-300, #fca5a5);
      font-size: 0.75rem;
      line-height: 1.35;
      overflow-wrap: anywhere;
      padding-left: 0.5rem;
    }
  `;
    documentRef.head.append(style);
}
