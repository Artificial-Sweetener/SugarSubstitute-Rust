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
/** Style the editing chrome with the host theme tokens shared by the canvas overlays. */

/**
 * Every interactive surface carries `sc-canvas-edit-chrome`, the class the native occlusion
 * reporter measures, so the native canvas yields pointer input exactly where chrome is shown.
 */
export function ensureCanvasEditStyles(document: Document): void {
  if (document.getElementById('sugarcubes-canvas-edit-styles')) return;
  const style = document.createElement('style');
  style.id = 'sugarcubes-canvas-edit-styles';
  style.textContent = `
    .sc-canvas-edit-root { position:absolute; inset:0; pointer-events:none; z-index:3;
      color:var(--fg-color,#ddd); font:13px var(--comfy-font-family,system-ui,sans-serif); }
    .sc-canvas-edit-dock { position:absolute; inset:0; pointer-events:none; }
    .sc-canvas-edit-topbar { position:absolute; left:0; top:0; display:flex; align-items:flex-start;
      gap:8px; pointer-events:none; }
    .sc-canvas-edit-rail-dock { position:absolute; left:0; top:0; display:flex; flex-direction:column;
      pointer-events:none; max-height:calc(100% - 16px); }
    .sc-canvas-edit-chrome { pointer-events:auto; box-sizing:border-box;
      border:1px solid var(--interface-stroke,#454545); border-radius:8px;
      background:var(--sugarcubes-card-surface,var(--comfy-menu-bg,#202020)); box-shadow:0 2px 8px #0003; }
    .sc-canvas-edit-row { display:flex; align-items:center; gap:4px; padding:3px; }
    .sc-canvas-edit-rail { display:flex; flex-direction:column; gap:2px; padding:2px; }
    .sc-canvas-edit-root button { color:inherit; font:inherit; cursor:pointer; box-sizing:border-box;
      display:inline-flex; align-items:center; justify-content:center; gap:6px; flex-shrink:0;
      min-height:28px; min-width:28px; padding:0 8px; border:1px solid transparent; border-radius:5px;
      background:transparent; }
    .sc-canvas-edit-root button:hover:not(:disabled) {
      background:color-mix(in srgb,var(--fg-color,#fff) 8%,transparent); }
    .sc-canvas-edit-root button:disabled { opacity:.4; cursor:default; }
    .sc-canvas-edit-root :focus-visible { outline:2px solid var(--p-primary-color,var(--ring,#df258e));
      outline-offset:1px; }
    .sc-canvas-edit-root button.sc-canvas-edit-icon-button { padding:0; width:28px; }
    .sc-canvas-edit-root button.sc-canvas-edit-primary { color:#fff;
      background:var(--p-primary-color,var(--ring,#df258e)); }
    .sc-canvas-edit-rail button { position:relative; width:34px; height:34px; padding:0; }
    .sc-canvas-edit-rail button[aria-pressed="true"] {
      background:color-mix(in srgb,var(--fg-color,#fff) 10%,transparent); }
    .sc-canvas-edit-rail button[aria-pressed="true"]::before { content:""; position:absolute; left:-1px;
      top:9px; width:3px; height:16px; border-radius:1.5px; background:var(--p-primary-color,var(--ring,#df258e)); }
    .sc-canvas-edit-rail button[aria-haspopup="menu"]::after { content:""; position:absolute; right:5px;
      bottom:5px; border-style:solid; border-width:0 0 5px 5px;
      border-color:transparent transparent var(--p-primary-color,var(--ring,#df258e)) transparent; }
    .sc-canvas-edit-icon { display:inline-block; width:20px; height:20px; flex:0 0 20px;
      background-color:currentColor; -webkit-mask:var(--sc-canvas-edit-icon) center / 20px 20px no-repeat;
      mask:var(--sc-canvas-edit-icon) center / 20px 20px no-repeat; }
    .sc-canvas-edit-label { font-weight:600; padding:0 6px; white-space:nowrap; }
    .sc-canvas-edit-options { display:flex; flex-direction:column; gap:4px; padding:3px; min-width:132px; }
    .sc-canvas-edit-options-header { display:flex; align-items:center; gap:0; }
    .sc-canvas-edit-options-header > button:first-child { flex:1 1 auto; justify-content:flex-start; }
    .sc-canvas-edit-tip { width:20px; height:20px; flex:0 0 20px; }
    .sc-canvas-edit-sliders { display:grid; grid-template-columns:auto 128px 48px; align-items:center;
      gap:4px 8px; padding:8px; }
    .sc-canvas-edit-sliders input[type="range"] { width:128px; margin:0;
      accent-color:var(--p-primary-color,var(--ring,#df258e)); }
    .sc-canvas-edit-coverage-controls { display:flex; align-items:center; gap:8px; }
    .sc-canvas-edit-coverage-controls select,.sc-canvas-edit-coverage-controls input {
      color:inherit; font:inherit; height:28px; box-sizing:border-box; border-radius:5px;
      border:1px solid var(--interface-stroke,#454545); background:var(--comfy-input-bg,#303030); }
    .sc-canvas-edit-coverage-controls input { width:42px; text-align:center; -moz-appearance:textfield; }
    .sc-canvas-edit-coverage-controls input::-webkit-inner-spin-button { display:none; }
    .sc-canvas-edit-toolbar { position:absolute; left:0; top:0; }
    .sc-canvas-edit-coverage { position:absolute; left:50%; bottom:8px; transform:translateX(-50%);
      display:flex; align-items:center; gap:8px; padding:3px; max-width:calc(100% - 16px); }
    .sc-canvas-edit-separator { width:1px; align-self:stretch; margin:4px 2px;
      background:var(--interface-stroke,#454545); }
  `;
  document.head.append(style);
}
