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
/** Own the themed-wide native suppression and floating scrollbar presentation. */
export function nativeScrollbarCss(): string {
  return `
    * { scrollbar-width:none!important; scrollbar-gutter:auto!important; }
    *::-webkit-scrollbar { display:none!important; width:0!important; height:0!important; }
    *::-webkit-scrollbar-button, *::-webkit-scrollbar-corner { display:none!important; }
    .p-scrollpanel-bar { display:none!important; }
  `;
}

/** Let viewport-clipped handles paint beyond a layer that moves with native scroll content. */
export function scrollbarPresentationCss(): string {
  return `
    .sc-style-scrollbar-layer {
      position:absolute; inset:0; z-index:1; overflow:visible; pointer-events:none;
    }
    .sc-style-scrollbar {
      position:absolute; box-sizing:border-box; pointer-events:auto; touch-action:none;
      background:transparent; outline:none;
    }
    .sc-style-scrollbar[hidden] { display:none!important; }
    .sc-style-scrollbar::before {
      content:''; position:absolute; border-radius:999px;
      background:var(--sc-style-scrollbar-thumb,rgba(255,255,255,.55));
      transition:width 150ms cubic-bezier(.2,0,0,1),height 150ms cubic-bezier(.2,0,0,1),background-color 150ms;
    }
    .sc-style-scrollbar[aria-orientation="vertical"]::before { width:calc(3px / var(--sc-scrollbar-scale-x,1)); height:100%; right:calc(1px / var(--sc-scrollbar-scale-x,1)); }
    .sc-style-scrollbar[aria-orientation="horizontal"]::before { height:calc(2px / var(--sc-scrollbar-scale-y,1)); width:100%; bottom:0; }
    .sc-style-scrollbar[aria-orientation="vertical"]:is(:hover,:focus-visible,[data-dragging])::before { width:calc(6px / var(--sc-scrollbar-scale-x,1)); }
    .sc-style-scrollbar[aria-orientation="horizontal"]:is(:hover,:focus-visible,[data-dragging])::before { height:calc(4px / var(--sc-scrollbar-scale-y,1)); }
    .sc-style-scrollbar:focus-visible::before { outline:1px solid var(--sc-accent-indicator,Highlight); outline-offset:1px; }
    @media (prefers-reduced-motion:reduce) { .sc-style-scrollbar::before { transition:none; } }
    @media (forced-colors:active) { .sc-style-scrollbar::before { background:ButtonText; } }
    @media (forced-colors:active) { .sc-style-scrollbar:focus-visible::before { outline-color:Highlight; } }
  `;
}

/** Paint the same overlay controls as Photoshop's square tracks in every PhotoPro palette. */
export function photoProScrollbarCss(): string {
  return `
    [data-sc-photopro-scroll-gutter] {
      padding-right:calc(var(--sc-scrollbar-base-padding-right,0px) + 14px) !important;
    }
    .sc-style-scrollbar[data-sc-photopro-scrollbar] { isolation:isolate; }
    .sc-style-scrollbar[data-sc-photopro-scrollbar] .sc-style-scrollbar-track {
      position:absolute; z-index:0; display:flex; box-sizing:border-box; justify-content:space-between;
      border:1px solid var(--sc-photopro-scroll-border,#1f1f1f); border-radius:0;
      background:var(--sc-photopro-scroll-track,#333); pointer-events:auto;
    }
    .sc-style-scrollbar[data-sc-photopro-scrollbar] .sc-style-scrollbar-track[hidden] { display:none!important; }
    .sc-style-scrollbar[data-sc-photopro-scrollbar][aria-orientation="vertical"] .sc-style-scrollbar-track { flex-direction:column; }
    .sc-style-scrollbar[data-sc-photopro-scrollbar][aria-orientation="horizontal"] .sc-style-scrollbar-track { flex-direction:row; }
    .sc-style-scrollbar[data-sc-photopro-scrollbar] .sc-style-scrollbar-track button {
      position:relative; flex:none; box-sizing:border-box; width:12px; height:12px; padding:0;
      border:0; border-radius:0; background:var(--sc-photopro-scroll-track,#333);
      color:var(--sc-photopro-scroll-arrow,#999); cursor:default;
    }
    .sc-style-scrollbar[data-sc-photopro-scrollbar] .sc-style-scrollbar-track button:hover { background:var(--sc-photopro-scroll-thumb,#555); }
    .sc-style-scrollbar[data-sc-photopro-scrollbar] .sc-style-scrollbar-track button::before {
      content:''; position:absolute; left:4px; top:4px; width:4px; height:4px;
      border-top:1px solid currentColor; border-left:1px solid currentColor;
    }
    .sc-style-scrollbar[data-sc-photopro-scrollbar][aria-orientation="vertical"] [data-sc-scroll-step="start"]::before { transform:rotate(45deg); }
    .sc-style-scrollbar[data-sc-photopro-scrollbar][aria-orientation="vertical"] [data-sc-scroll-step="end"]::before { transform:rotate(225deg); }
    .sc-style-scrollbar[data-sc-photopro-scrollbar][aria-orientation="horizontal"] [data-sc-scroll-step="start"]::before { transform:rotate(-45deg); }
    .sc-style-scrollbar[data-sc-photopro-scrollbar][aria-orientation="horizontal"] [data-sc-scroll-step="end"]::before { transform:rotate(135deg); }
    .sc-style-scrollbar[data-sc-photopro-scrollbar]::before {
      z-index:1; box-sizing:border-box; border:1px solid var(--sc-photopro-scroll-border,#1f1f1f); border-radius:0;
      background:var(--sc-photopro-scroll-thumb,#555); transition:none;
    }
    .sc-style-scrollbar[data-sc-photopro-scrollbar][aria-orientation="vertical"]::before,
    .sc-style-scrollbar[data-sc-photopro-scrollbar][aria-orientation="vertical"]:is(:hover,:focus-visible,[data-dragging])::before {
      width:calc(10px / var(--sc-scrollbar-scale-x,1)); height:100%; right:calc(2px / var(--sc-scrollbar-scale-x,1));
    }
    .sc-style-scrollbar[data-sc-photopro-scrollbar][aria-orientation="horizontal"]::before,
    .sc-style-scrollbar[data-sc-photopro-scrollbar][aria-orientation="horizontal"]:is(:hover,:focus-visible,[data-dragging])::before {
      height:calc(10px / var(--sc-scrollbar-scale-y,1)); width:100%; bottom:calc(2px / var(--sc-scrollbar-scale-y,1));
    }
  `;
}

/** Apply the current palette without overriding the native owner's stacking context. */
export function scrollbarCss(dark: boolean): string {
  return `${nativeScrollbarCss()} :root { --sc-style-scrollbar-thumb:${dark ? 'rgba(255,255,255,.55)' : 'rgba(0,0,0,.45)'}; } ${scrollbarPresentationCss()} ${photoProScrollbarCss()}`;
}
