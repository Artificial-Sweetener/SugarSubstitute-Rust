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
/** Scope Fluent finder chrome and transient search decoration to the owned masonry viewport. */
/** Install static styles once without affecting native Comfy toolbars or graph nodes. */
export function ensureFinderStyles(documentRef: Document): void {
  if (documentRef.getElementById('sc-masonry-finder-styles')) return;
  const style = documentRef.createElement('style');
  style.id = 'sc-masonry-finder-styles';
  style.textContent = `
    .sc-masonry-finder {
      position:absolute; top:16px; left:50%; transform:translateX(-50%); z-index:1200; pointer-events:auto;
      display:flex; align-items:center; gap:2px; box-sizing:border-box; padding:6px 8px;
      max-width:calc(100% - 16px); border:1px solid var(--colorNeutralStroke2);
      border-radius:8px; background:var(--colorNeutralBackground3); color:var(--colorNeutralForeground1);
      font-family:var(--fontFamilyBase); font-size:var(--fontSizeBase300); line-height:var(--lineHeightBase300);
    }
    .sc-masonry-finder[hidden], .sc-masonry-finder [hidden],
    [data-sc-search-card-hidden], [data-sc-search-field-hidden] { display:none !important; }
    .sc-masonry-finder__input { display:flex; height:32px; width:296px; min-width:0; flex:1 1 296px; }
    .sc-masonry-finder__input::part(root) { padding:0 2px 0 0; gap:0; }
    .sc-masonry-finder__input::part(control) { min-width:0; padding:0 6px; }
    .sc-masonry-finder fluent-button.sc-masonry-finder__mode { width:80px; height:30px; flex:0 0 80px; padding:0 10px; font-size:14px; font-weight:400; border-radius:4px 0 0 4px; }
    .sc-masonry-finder__mode::part(content) { width:100%; align-items:center; gap:10px; justify-content:space-between; }
    .sc-masonry-finder__chevron { display:block; width:6px; height:6px; border-right:1px solid currentColor; border-bottom:1px solid currentColor; transform:translateY(-2px) rotate(45deg); opacity:.75; }
    .sc-masonry-finder__input-actions { display:flex; align-items:center; gap:0; }
    .sc-masonry-finder fluent-button { width:28px; height:28px; min-width:28px; min-height:28px; flex:0 0 28px; padding:0; }
    .sc-masonry-finder fluent-button::part(control) { width:28px; min-width:28px; height:28px; padding:0; }
    .sc-masonry-finder__icon { display:block; width:16px; height:16px; background:currentColor; mask:var(--sc-finder-icon) center/contain no-repeat; }
    .sc-masonry-finder__status { position:absolute; width:1px; height:1px; overflow:hidden; clip-path:inset(50%); }
    .sc-search-highlight { position:absolute; pointer-events:none; overflow:hidden; color:transparent !important; z-index:1; }
    .sc-search-highlight__text { color:transparent !important; }
    .sc-search-highlight mark { color:transparent !important; background:color-mix(in srgb,var(--colorBrandBackground,#479ef5) 35%,transparent); padding:0; }
    .sc-search-highlight mark[data-active] { background:color-mix(in srgb,var(--colorBrandBackground,#479ef5) 59%,transparent); }
  `;
  documentRef.head.append(style);
}
