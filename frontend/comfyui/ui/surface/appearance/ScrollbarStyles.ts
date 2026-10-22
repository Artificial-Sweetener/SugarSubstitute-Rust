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

/** Apply the current palette without overriding the native owner's stacking context. */
export function scrollbarCss(dark: boolean): string {
  return `${nativeScrollbarCss()} :root { --sc-style-scrollbar-thumb:${dark ? 'rgba(255,255,255,.55)' : 'rgba(0,0,0,.45)'}; } ${scrollbarPresentationCss()}`;
}
