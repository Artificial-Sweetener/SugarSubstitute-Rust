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
/** Match Substitute's seed editor geometry and shared Fluent interaction tokens. */
/** Style only recognized native seed fields in both graph and masonry. */
export function fluentSeedCss() {
    return `
    [data-sc-number-spinner][data-sc-seed-box] { width:100%; max-width:100%; min-width:60px; padding-right:46px; overflow:visible; font-variant-numeric:tabular-nums; }
    .sugarcubes-workflow-masonry [data-sc-number-spinner][data-sc-seed-box] { width:calc(20ch + 64px); }
    .sugarcubes-workflow-masonry [data-sc-fluent-row][data-sc-fluent-widget^="WidgetInputNumber"] [data-sc-fluent-layout] > :last-child:has([data-sc-seed-box]) { flex:0 0 calc(20ch + 64px); width:calc(20ch + 64px); max-width:100%; }
    [data-sc-seed-box] [data-sc-number-increment], [data-sc-seed-box] [data-sc-number-decrement],
    [data-sc-seed-box] [data-testid="value-control"] { display:none; }
    [data-sc-seed-box] [data-sc-number-input] { text-align:left; font:inherit; }
    [data-sc-seed-split] { position:absolute; right:0; top:0; bottom:0; width:46px; display:flex; }
    [data-sc-seed-split] > button { padding:0; min-width:0; height:100%; border:0; border-radius:0; background:transparent; color:var(--colorNeutralForeground1); display:flex; align-items:center; justify-content:center; cursor:default; transition:background-color var(--durationFast) var(--curveEasyEase); }
    [data-sc-seed-toggle] { width:28px; }
    [data-sc-seed-menu] { width:18px; border-radius:0 4px 4px 0 !important; }
    [data-sc-seed-menu] > span { width:12px; height:12px; }
    [data-sc-seed-icon] { width:16px; height:16px; flex:none; background:currentColor; mask:var(--sc-seed-icon) center / contain no-repeat; }
    [data-sc-seed-split] button:hover { background:var(--colorNeutralBackground1Hover); }
    [data-sc-seed-split] button:active { background:var(--colorNeutralBackground1Pressed); }
    [data-sc-seed-split] button:focus-visible { outline:2px solid var(--colorBrandStroke1); outline-offset:-2px; }
    [data-sc-seed-split] button:disabled { opacity:.4; }
    [data-sc-seed-options] { box-sizing:border-box; position:absolute; right:0; top:calc(100% + 4px); z-index:100; width:158px; min-width:158px; padding:4px; border-radius:6px; border:1px solid var(--colorNeutralStroke2); background:var(--colorNeutralBackground1); box-shadow:var(--shadow16); }
    [data-sc-seed-options][hidden] { display:none; }
    [data-sc-seed-options] > button { display:flex; align-items:center; gap:10px; width:100%; padding:7px 10px; border:0; border-radius:4px; text-align:left; background:transparent; color:var(--colorNeutralForeground1); font:inherit; }
    [data-sc-seed-options] > button[aria-checked="true"] { box-shadow:inset 3px 0 var(--colorBrandStroke1); }
  `;
}
