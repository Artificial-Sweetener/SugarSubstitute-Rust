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
/** Scope themed finder chrome and transient search decoration to the owned masonry viewport. */
import { fieldUnderlineCss } from '../../surface/appearance/FieldSurfaceStyles.js';

const acrylicGrain = new URL('../../../../textures/acrylic-grain.svg', import.meta.url).href;
/** Install static styles once without affecting native Comfy toolbars or graph nodes. */
export function ensureFinderStyles(documentRef: Document): void {
  if (documentRef.getElementById('sc-masonry-finder-styles')) return;
  const style = documentRef.createElement('style');
  style.id = 'sc-masonry-finder-styles';
  style.textContent = `
    .sc-masonry-finder {
      position:absolute; top:16px; left:50%; transform:translateX(-50%); z-index:1200; pointer-events:auto;
      display:flex; align-items:center; gap:2px; box-sizing:border-box; padding:6px 8px;
      max-width:calc(100% - 16px); border:1px solid var(--sc-border-subtle);
      border-radius:8px; background:color-mix(in srgb,var(--sc-surface-secondary) 78%,transparent); color:var(--sc-text);
      backdrop-filter:blur(30px) saturate(125%); isolation:isolate; box-shadow:var(--sc-shadow-medium);
      font-family:var(--sc-font-family); font-size:var(--sc-font-size); line-height:var(--sc-line-height);
    }
    .sc-masonry-finder::before { content:''; position:absolute; inset:0; z-index:-1; pointer-events:none; border-radius:inherit; background:url("${acrylicGrain}") repeat; opacity:.035; }
    .sc-masonry-finder[hidden], .sc-masonry-finder [hidden],
    [data-sc-search-card-hidden], [data-sc-search-field-hidden] { display:none !important; }
    .sc-masonry-finder__group { display:flex; align-items:center; height:32px; width:296px; min-width:0; flex:1 1 296px; border:1px solid var(--sc-border); border-radius:var(--sc-control-radius); background:var(--sc-surface); }
    .sc-masonry-finder__input { display:flex; height:30px; width:100%; min-width:0; flex:1; --mod-textfield-background-color:transparent; --mod-textfield-border-width:0px; }
    .sc-masonry-finder__input::part(root) { padding:0; gap:0; border:0; background:transparent; }
    /* The whole group is the finder's one field: the shared focus underline spans it, cut by its
       corners, and the text input inside draws none of its own. */
    .sc-masonry-finder__input::part(root)::after { display:none; }
    .sc-masonry-finder__group { position:relative; }
    ${fieldUnderlineCss('.sc-masonry-finder__group', '.sc-masonry-finder__group:focus-within')}
    .sc-masonry-finder__input::part(control) { min-width:0; padding:0 6px; }
    .sc-masonry-finder [data-sc-button].sc-masonry-finder__mode { width:80px; height:30px; flex:0 0 80px; padding:0 10px; font-size:14px; font-weight:400; border-radius:4px 0 0 4px; border-right-width:1px; border-right-style:solid; border-right-color:var(--sc-border-subtle); }
    .sc-masonry-finder__mode::part(content) { width:100%; align-items:center; gap:10px; justify-content:space-between; }
    .sc-masonry-finder__mode-content { display:flex; align-items:center; justify-content:space-between; gap:10px; width:100%; }
    .sc-masonry-finder__chevron { display:block; width:6px; height:6px; border-right:1px solid currentColor; border-bottom:1px solid currentColor; transform:translateY(-2px) rotate(45deg); opacity:.75; }
    .sc-masonry-finder__input-actions { display:flex; align-items:center; gap:2px; }
    .sc-masonry-finder [data-sc-button] { width:28px; height:28px; min-width:28px; min-height:28px; flex:0 0 28px; padding:0; border-radius:var(--sc-control-radius); }
    .sc-masonry-finder__icon { display:block; width:16px; height:16px; background:currentColor; mask:var(--sc-finder-icon) center/contain no-repeat; }
    .sc-masonry-finder__status { position:absolute; width:1px; height:1px; overflow:hidden; clip-path:inset(50%); }
    /* A text box's transparent copy. Its matches are runs of the copy, filled to the line as the
       box's own selection is, in the box's ink; their colours are the search highlights', set
       once beside the selection they share a look with (the accent theme). */
    .sc-search-highlight { position:absolute; pointer-events:none; overflow:hidden; color:transparent !important; z-index:1; }
    .sc-search-highlight__text { color:transparent !important; }
    .sc-search-highlight [data-sc-search] { padding-block:var(--sc-search-line-fill,0px); color:var(--sc-search-mirror-ink); }
    .sc-search-highlight [data-sc-search="current"] { color:var(--sc-selection-ink,var(--sc-search-mirror-ink)); }
    @supports not (backdrop-filter:blur(1px)) {
      .sc-masonry-finder { background:var(--sc-surface-secondary); }
      .sc-masonry-finder::before { display:none; }
    }
    @media (prefers-reduced-transparency:reduce) {
      .sc-masonry-finder { background:var(--sc-surface-secondary); backdrop-filter:none; }
      .sc-masonry-finder::before { display:none; }
    }
    @media (forced-colors:active) {
      .sc-masonry-finder { background:Canvas; border-color:CanvasText; backdrop-filter:none; }
      .sc-masonry-finder::before { display:none; }
    }
  `;
  documentRef.head.append(style);
}
