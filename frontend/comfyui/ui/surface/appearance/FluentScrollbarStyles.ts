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
/** Own the Fluent-wide native suppression and floating scrollbar presentation. */
export function fluentNativeScrollbarCss(): string {
  return `
    * { scrollbar-width:none!important; scrollbar-gutter:auto!important; }
    *::-webkit-scrollbar { display:none!important; width:0!important; height:0!important; }
    *::-webkit-scrollbar-button, *::-webkit-scrollbar-corner { display:none!important; }
    .p-scrollpanel-bar { display:none!important; }
  `;
}

/** Render trackless handles without allocating any space in their scroll owners. */
export function fluentScrollbarCss(dark: boolean): string {
  return `${fluentNativeScrollbarCss()}
    .sc-fluent-scrollbar-layer {
      position:fixed; inset:0; z-index:2147483646; pointer-events:none;
    }
    .sc-fluent-scrollbar {
      position:absolute; box-sizing:border-box; pointer-events:auto; touch-action:none;
      background:transparent; outline:none;
    }
    .sc-fluent-scrollbar[hidden] { display:none!important; }
    .sc-fluent-scrollbar::before {
      content:''; position:absolute; border-radius:999px;
      background:${dark ? 'rgba(255,255,255,.55)' : 'rgba(0,0,0,.45)'};
      transition:width 150ms cubic-bezier(.2,0,0,1),height 150ms cubic-bezier(.2,0,0,1),background-color 150ms;
    }
    .sc-fluent-scrollbar[aria-orientation="vertical"]::before { width:3px; height:100%; right:1px; }
    .sc-fluent-scrollbar[aria-orientation="horizontal"]::before { height:3px; width:100%; bottom:1px; }
    .sc-fluent-scrollbar[aria-orientation="vertical"]:is(:hover,:focus-visible,[data-dragging])::before { width:6px; }
    .sc-fluent-scrollbar[aria-orientation="horizontal"]:is(:hover,:focus-visible,[data-dragging])::before { height:6px; }
    .sc-fluent-scrollbar:focus-visible::before { outline:1px solid Highlight; outline-offset:1px; }
    @media (prefers-reduced-motion:reduce) { .sc-fluent-scrollbar::before { transition:none; } }
    @media (forced-colors:active) { .sc-fluent-scrollbar::before { background:ButtonText; } }
  `;
}
