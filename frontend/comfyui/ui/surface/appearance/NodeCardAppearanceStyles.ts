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
/** Own the default Cube-face visual treatment independently of layout geometry. */
export function ensureNodeCardAppearanceStyles(documentRef: Document): void {
  if (documentRef.getElementById('sugarcubes-node-card-appearance')) return;
  const style = documentRef.createElement('style');
  style.id = 'sugarcubes-node-card-appearance';
  style.textContent = `
    .sugarcubes-cube-face__node-card { box-sizing:border-box; border:1px solid var(--interface-stroke,#454545); border-radius:12px; background:var(--sugarcubes-masonry-card-surface,var(--comfy-input-bg,#222)); }
    .sugarcubes-cube-face__node-card .lg-node { cursor:default; }
    .sugarcubes-cube-face__node-card .lg-node:not([data-sc-fluent-node]) [data-testid="advanced-inputs-button"] {
      --node-component-header-surface:var(--sugarcubes-cube-card-header, var(--sugarcubes-masonry-card-surface, var(--comfy-input-bg, #222)));
    }
    .sugarcubes-cube-face__node-card [data-masonry-bypass-badge] { display:none !important; }
    .sugarcubes-cube-face__node-card [data-masonry-title-icon] {
      position:relative; padding-left:30px;
    }
    .sugarcubes-workflow-masonry__node-icon {
      position:absolute; left:0; top:50%; transform:translateY(-50%);
      width:24px; height:24px; pointer-events:none;
    }
    .sugarcubes-workflow-masonry__node-icon::before {
      content:''; display:block; width:20px; height:20px; margin:2px;
      background:currentColor; mask:var(--sc-node-icon) center / contain no-repeat;
    }
    .sugarcubes-cube-face__node-card .lg-node.outline-node-component-outline { outline-color:transparent; }
    .sugarcubes-cube-face__node-card .lg-node > [data-testid="node-state-outline-overlay"].border-node-component-outline {
      border-color:transparent;
    }

    .sugarcubes-native-node-card .lg-node:not([data-sc-fluent-node]) [data-testid="node-inner-wrapper"] {
      background-color:var(--sugarcubes-cube-card-header) !important;
    }
    .sugarcubes-native-node-card .lg-node:not([data-sc-fluent-node]) [data-testid^="node-body-"] {
      background-color:var(--sugarcubes-cube-card-body) !important;
    }
  `;
  documentRef.head.append(style);
}
