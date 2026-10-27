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
/** Compose GIMP panel and graph paint around the existing native layout and interaction owners. */
import { nativeAppearanceCss } from './NativeAppearanceStyles.js';
import { nativeFieldLayoutCss } from './NativeFieldLayoutStyles.js';
import { gimpTokenDeclarations } from './GimpThemeTokens.js';
import { gimpWorkspaceCss } from './GimpWorkspaceStyles.js';
import { gimpNumericCss } from './GimpNumericStyles.js';
import { gimpIconCss } from './GimpIconStyles.js';
import { gimpComboCss } from './GimpComboStyles.js';

/** Keep the graph's native structure while applying GIMP's compact Default theme. */
export function gimpAppearanceCss(dark: boolean): string {
  return (
    nativeAppearanceCss(
      {
        tokens: gimpTokenDeclarations(dark),
        canvas: 'var(--sc-gimp-background)',
        cardFill: 'var(--sc-gimp-background)',
        cardStroke: 'var(--sc-gimp-border)',
        rowStroke: 'transparent',
        controlFill: 'var(--sc-gimp-widget)',
        controlStroke: 'var(--sc-gimp-strongerBorder)',
        controlBottomStroke: 'var(--sc-gimp-strongerBorder)',
        radius: '3px',
      },
      dark,
    ) +
    nativeFieldLayoutCss('html[data-sc-visual-style="gimp"] :where([data-sc-style-node])') +
    gimpSurfaceCss()
  );
}

/** Style owned surfaces independently of renderer availability, including floating menus and dialogs. */
export function gimpSurfaceCss(): string {
  const scope = 'html[data-sc-visual-style="gimp"]';
  const controls = `${scope} :is([data-sc-theme="gimp"],[data-sc-style-node],[data-sc-style-surface],[data-sc-style-popup])`;
  return (
    `
    ${controls} { font-family:var(--sc-font-family); color:var(--sc-text); }
    ${controls} :is(button,input,textarea,[role="combobox"]) { font-family:var(--sc-font-family); font-size:12px; }
    ${controls} :is(input,textarea,[role="combobox"]):not([type="checkbox"]):not([type="range"]):not([data-sc-number-input]):not([data-sc-style-combo-frame] *) { border-radius:3px; background:var(--sc-gimp-widget); color:var(--sc-gimp-foreground); }
    ${controls} :is(button,input,textarea,[role="combobox"],[role="tab"]):focus-visible:not([data-sc-style-combo-frame] *) { outline:1px dotted var(--sc-gimp-foreground); outline-offset:-3px; }
    ${scope} [data-sc-style-node] { --sc-node-card-radius:3px; }
    ${scope} [data-sc-style-node] [data-testid^="node-header-"] { box-sizing:border-box; min-height:32px; height:32px; border-bottom:1px solid var(--sc-gimp-strongBorder); background:var(--sc-gimp-widget); }
    ${scope} [data-sc-style-node] [data-testid="node-title"] { font-size:12px; font-weight:600; }
    ${scope} [data-sc-style-node] [data-testid="node-title"] .editable-text { font-weight:600; }
    ${scope} [data-sc-style-node] [data-sc-style-row], ${scope}[data-sc-compact-masonry] [data-sc-style-node] [data-sc-style-row] { min-height:28px; padding-top:1px; padding-bottom:1px; border-color:transparent; }
    ${scope} .sugarcubes-cube-face__node-card [data-sc-style-row] > [data-sc-style-control], ${scope} .sugarcubes-cube-face__node-card [data-sc-style-layout] { min-height:var(--sc-control-height); }
    ${scope} [data-sc-style-row] ~ [data-sc-style-row] { border-top-color:transparent; }
    ${scope} [data-sc-boolean-checkbox] { display:inline-flex; align-items:center; width:14px; height:14px; flex:none; }
    ${scope} [data-sc-boolean-segments] { display:flex; height:var(--sc-control-height); gap:0; }
    ${scope} [data-sc-boolean-segment] { flex:1; min-width:28px; }
    ${scope} sc-gimp-search { height:var(--sc-control-height); width:100%; }
    ${scope} [data-sc-titlebar-controls] [data-cube-card-activation] { margin-left:8px; padding:0; }
    ${scope} [data-sc-titlebar-controls] > :first-child { justify-content:flex-start; }
    ${scope} [data-sc-titlebar-controls] > :first-child > :has(> [data-testid="node-title"]) { flex:0 1 auto; margin-right:0; }
    ${scope} [data-sc-editor-field="gimp"] { --sc-editor-content-padding:4px 6px; --sc-editor-content-padding-left:6px; }
    ${scope} [data-sc-prompt-style="gimp"] .cm-editor { font:12px/18px var(--sc-font-family); }
    ${scope} :is([data-sc-menu-surface="actions"],[data-sc-menu-surface="choices"][data-sc-style-popup]) { padding:3px; border:1px solid var(--sc-gimp-border); border-radius:3px; background:var(--sc-gimp-background); color:var(--sc-gimp-foreground); box-shadow:var(--sc-shadow-medium); font:12px/18px var(--sc-font-family); }
    ${scope} [data-sc-menu-surface] :is([role^="menuitem"],[role="option"]) { display:flex; align-items:center; column-gap:6px; position:relative; min-height:24px; padding:3px 24px; border-radius:2px; background:transparent; color:inherit; }
    ${scope} [data-sc-menu-surface="actions"] [role^="menuitem"] > [aria-hidden="true"]:first-child { flex:none; align-self:center; line-height:1; }
    ${scope} [data-sc-menu-surface] :is([role^="menuitem"],[role="option"]):is(:hover,:focus-visible,[data-highlighted]) { background:var(--sc-gimp-selected); outline:1px dotted var(--sc-gimp-foreground); outline-offset:-3px; }
    ${scope} [data-sc-menu-surface] [aria-checked="true"]::before { content:'✓'; position:absolute; left:6px; top:50%; transform:translateY(-50%); font-size:14px; line-height:1; }
    ${scope} [data-sc-menu-surface] [aria-disabled="true"] { color:var(--sc-gimp-disabled); }
  ` +
    gimpWorkspaceCss() +
    gimpNumericCss() +
    gimpComboCss() +
    gimpIconCss()
  );
}
