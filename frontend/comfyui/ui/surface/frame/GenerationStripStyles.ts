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
/** Match Substitute's top-bleeding generation cluster and overlapping numeric accessory. */
export const GENERATION_STRIP_WIDTH = 226;

/** Keep strip styling confined to the explicitly enabled Substitute frame. */
export const generationStripCss = `
  .sc-substitute-frame[data-generation-strip] { padding-right:calc(${GENERATION_STRIP_WIDTH}px + var(--sc-native-caption-width,0px)); }
  :root[data-sc-substitute-frame] [data-sc-generation-native] { display:none !important; }
  .sc-generation-strip { position:absolute; top:0; right:var(--sc-native-caption-width,0px); width:${GENERATION_STRIP_WIDTH}px; height:32px;
    display:flex; align-items:flex-start; -webkit-app-region:no-drag; font-family:var(--fontFamilyBase); }
  .sc-generation-batch { position:relative; width:72px; height:30px; flex:0 0 72px;
    background:rgba(32,32,32,.588); border-radius:0 0 0 6px; color:white; }
  .sc-generation-batch input { position:absolute; left:4px; top:2px; width:38px; height:26px; box-sizing:border-box;
    background:transparent; color:inherit; border:0; padding:0; text-align:center; font:600 13px var(--fontFamilyBase);
    appearance:textfield; outline:none; }
  .sc-generation-batch input:focus-visible { outline:1px solid var(--sc-accent-color); border-radius:3px; }
  .sc-generation-batch input::-webkit-inner-spin-button { appearance:none; }
  .sc-generation-step { position:absolute; left:44px; width:14px; height:15px; border:0; padding:0;
    background:transparent; color:inherit; display:grid; place-items:center; cursor:pointer; }
  .sc-generation-step[data-direction='up'] { top:0; }
  .sc-generation-step[data-direction='down'] { top:15px; }
  .sc-generation-step span { width:6px; height:4px; background:currentColor; clip-path:polygon(0 100%,50% 0,100% 100%,84% 100%,50% 34%,16% 100%); }
  .sc-generation-step[data-direction='down'] span { transform:rotate(180deg); }
  .sc-generation-step:hover { background:rgba(255,255,255,.086); }
  .sc-generation-step:active { background:rgba(255,255,255,.133); }
  .sc-generation-step:disabled { opacity:.36; cursor:default; }
  .sc-generation-actions { display:flex; width:160px; height:30px; margin-left:-6px; position:relative;
    background:var(--sc-accent-color); border-radius:0 0 6px 6px; overflow:hidden; }
  .sc-generation-action { position:relative; width:40px; height:30px; flex:0 0 40px; padding:0; border:0;
    display:grid; place-items:center; background:transparent; color:black; cursor:pointer; }
  .sc-generation-action + .sc-generation-action::before { content:''; position:absolute; left:0; top:5px; bottom:5px;
    width:1px; background:rgba(0,0,0,.322); }
  .sc-generation-action:hover { background:rgba(255,255,255,.118); }
  .sc-generation-action:active { background:rgba(0,0,0,.125); }
  .sc-generation-action:focus-visible,.sc-generation-step:focus-visible { outline:2px solid currentColor; outline-offset:-3px; }
  .sc-generation-action:disabled { background:rgb(52,52,52); color:rgba(255,255,255,.431); cursor:default; }
  .sc-generation-glyph { display:block; width:16px; height:16px; background:currentColor; mask:var(--sc-generation-glyph) center/contain no-repeat; }
  .sc-generation-action[data-action='skip'] .sc-generation-glyph { width:20px; height:20px; }
  .sc-generation-queue-slot { display:block; flex:0 0 40px; width:40px; height:30px; }
  :root[data-sc-substitute-frame] button[data-sc-generation-queue] { position:fixed!important; left:var(--sc-queue-left)!important;
    top:var(--sc-queue-top)!important; width:40px!important; height:30px!important; min-width:40px!important; padding:0!important;
    border:0!important; border-radius:0 0 6px 0!important; box-shadow:none!important; z-index:1100; font-size:0!important;
    display:grid!important; place-items:center!important; background:var(--sc-accent-color)!important; color:black!important; }
  :root[data-sc-substitute-frame] button[data-sc-generation-queue][data-sc-queue-theme='light'] { color:white!important; }
  :root[data-sc-substitute-frame] button[data-sc-generation-queue] > :not(.sc-generation-queue-glyph,.sc-generation-queue-badge) { display:none!important; }
  :root[data-sc-substitute-frame] button[data-sc-generation-queue]::before { content:''; position:absolute; left:0; top:5px; bottom:5px;
    width:1px; background:rgba(0,0,0,.322); }
  :root[data-sc-substitute-frame] button[data-sc-generation-queue]:hover { background:color-mix(in srgb,var(--sc-accent-color),white 11.8%)!important; }
  :root[data-sc-substitute-frame] button[data-sc-generation-queue]:active { background:color-mix(in srgb,var(--sc-accent-color),black 12.5%)!important; }
  .sc-generation-queue-glyph { display:block; width:16px; height:16px; background:currentColor; mask:var(--sc-queue-glyph) center/contain no-repeat; pointer-events:none; }
  .sc-generation-queue-badge { position:absolute; right:3px; bottom:2px; min-width:12px; height:12px; padding:0 3px; border-radius:6px;
    background:black; color:var(--sc-accent-color); font:700 8px/12px var(--fontFamilyBase); pointer-events:none; }
  button[data-sc-queue-theme='light'] .sc-generation-queue-badge { background:white; }
  .sc-generation-queue-badge[hidden] { display:none!important; }
  .sc-substitute-frame[data-color-scheme='light'] .sc-generation-batch { background:rgba(251,251,251,.737); color:black; }
  .sc-substitute-frame[data-color-scheme='light'] .sc-generation-action { color:white; }
  .sc-substitute-frame[data-color-scheme='light'] .sc-generation-action:disabled { background:rgb(205,205,205); color:rgba(255,255,255,.902); }
`;
