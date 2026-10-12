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
/** Style reusable canvas controls in the owning overlay document. */

/** Install one shared static stylesheet per document without creating image or host dependencies. */
export function ensureCanvasOverlayStyles(document: Document): void {
  if (document.getElementById('sugarcubes-output-overlay-styles')) return;
  const style = document.createElement('style');
  style.id = 'sugarcubes-output-overlay-styles';
  style.textContent = `
    .sc-output-overlay-root,.sc-output-overlay { position:absolute; inset:0; pointer-events:none; }
    .sc-output-overlay { color:var(--fg-color,#ddd); font:13px var(--comfy-font-family,system-ui,sans-serif); }
    .sc-output-toolbar,.sc-output-comparison,.sc-output-status,.sc-output-tabs {
      pointer-events:auto; display:flex; align-items:center; gap:6px;
      border:1px solid var(--interface-stroke,#454545); border-radius:8px; padding:6px;
      background:var(--sugarcubes-card-surface,var(--comfy-menu-bg,#202020));
      box-shadow:0 2px 8px #0003; box-sizing:border-box;
    }
    .sc-output-navigation { position:absolute; top:12px; left:12px; right:12px; display:flex; flex-direction:column; gap:6px; }
    .sc-output-toolbar,.sc-output-comparison { flex-wrap:wrap; }
    .sc-output-status { position:absolute; bottom:64px; left:12px; right:12px; flex-wrap:wrap; font-size:12px; }
    .sc-output-tabs { position:absolute; bottom:12px; left:50%; transform:translateX(-50%);
      max-width:calc(100% - 24px); overflow-x:auto; white-space:nowrap; overscroll-behavior:contain; }
    .sc-output-overlay button,.sc-output-overlay select,.sc-output-overlay input {
      color:inherit; font:inherit; background:var(--comfy-input-bg,#303030);
      border:1px solid var(--interface-stroke,#454545); border-radius:5px; min-height:28px;
      padding:4px 8px; box-sizing:border-box;
    }
    .sc-output-overlay button { cursor:pointer; flex-shrink:0; }
    .sc-output-overlay button:hover:not(:disabled) { background:var(--secondary-background-hover,#444); }
    .sc-output-overlay button:disabled { opacity:.45; cursor:default; }
    .sc-output-overlay button[aria-pressed="true"],.sc-output-overlay [aria-selected="true"] {
      border-color:var(--p-primary-color,var(--ring,#df258e));
      box-shadow:inset 0 -2px var(--p-primary-color,var(--ring,#df258e));
    }
    .sc-output-overlay :focus-visible { outline:2px solid var(--p-primary-color,var(--ring,#df258e)); outline-offset:2px; }
    .sc-output-comparison select { min-width:0; max-width:60%; }
    .sc-output-comparison input { width:64px; }
    .sc-output-status span:empty { display:none; }
    .sc-output-announcement { position:absolute; width:1px; height:1px; padding:0; margin:-1px;
      overflow:hidden; clip:rect(0,0,0,0); white-space:nowrap; border:0; }
  `;
  document.head.append(style);
}
