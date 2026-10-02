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
/** Install layout structure independently of native node theme tokens. */
export function ensureWorkflowMasonryStyles(documentRef: Document): void {
  if (documentRef.getElementById('sugarcubes-workflow-masonry-styles')) return;
  const style = documentRef.createElement('style');
  style.id = 'sugarcubes-workflow-masonry-styles';
  style.textContent = `
    .sugarcubes-masonry-toggle {
      position:absolute; top:12px; right:16px; z-index:1500; pointer-events:auto;
      padding:8px 12px; border-radius:8px; cursor:pointer;
      border:1px solid var(--interface-stroke,#454545);
      background:var(--comfy-menu-bg,#202020); color:var(--fg-color,#ddd);
      font:inherit; font-size:13px;
    }
    .sugarcubes-masonry-toggle[aria-pressed="true"] { background:var(--interface-panel-selected-surface,#404040); }
    .sugarcubes-masonry-toggle:disabled { opacity:.5; cursor:default; }
    .sugarcubes-workflow-masonry {
      position:absolute; inset:0; z-index:1100; box-sizing:border-box; pointer-events:auto;
      overflow-y:auto; overflow-x:hidden; overscroll-behavior:contain;
      padding:52px 20px 96px; scrollbar-gutter:stable;
      background:var(--sugarcubes-masonry-surface,var(--comfy-menu-bg,#202020));
      color:var(--fg-color,#ddd); outline:none;
    }
    .sugarcubes-workflow-masonry__content { position:relative; width:100%; }
    .sugarcubes-workflow-masonry__section {
      position:absolute; left:0; right:0; height:36px;
      display:flex; align-items:center; gap:12px;
      border-bottom:1px solid var(--interface-stroke,#454545);
    }
    .sugarcubes-workflow-masonry__section h2 { font:inherit; font-weight:600; margin:0; }
    .sugarcubes-workflow-masonry__section small { opacity:.65; }
    .sugarcubes-workflow-masonry__actions { margin-left:auto; display:flex; gap:12px; align-items:center; }
    .sugarcubes-workflow-masonry__section button { color:inherit; background:transparent; border:0; cursor:pointer; }
    .sugarcubes-workflow-masonry__content > .sugarcubes-workflow-masonry__card {
      position:absolute; left:0; top:0; box-sizing:border-box;
      contain:layout style;
      border:1px solid var(--interface-stroke,#454545); border-radius:12px;
      background:var(--sugarcubes-masonry-card-surface,var(--comfy-input-bg,#222));
    }
    .sugarcubes-workflow-masonry__card .lg-node { cursor:default; }
    .sugarcubes-workflow-masonry__card .lg-node-content { content-visibility:visible; }
    .sugarcubes-workflow-masonry__card:has([data-reka-popper-content-wrapper]) { z-index:10; }
    #graph-canvas-container[data-sugarcubes-masonry="true"] > #graph-canvas,
    #graph-canvas-container[data-sugarcubes-masonry="true"] > .ph-no-capture,
    #graph-canvas-container[data-sugarcubes-masonry="true"] > .ph-no-capture *,
    #graph-canvas-container[data-sugarcubes-masonry="true"] > .isolate,
    #graph-canvas-container[data-sugarcubes-masonry="true"] > .isolate *,
    .graph-canvas-panel[data-sugarcubes-masonry="true"] [aria-label="Canvas Toolbar"],
    .graph-canvas-panel[data-sugarcubes-masonry="true"] .litegraph-minimap {
      visibility:hidden !important; pointer-events:none !important;
    }
    .sugarcubes-workflow-masonry__empty { padding:32px; opacity:.7; }
  `;
  documentRef.head.append(style);
}
