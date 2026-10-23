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
/** Apply OpenPhoto's compact paint to existing SugarCubes surfaces and layout. */
import { nativeAppearanceCss } from './NativeAppearanceStyles.js';
import { photoProComboCss } from './PhotoProComboStyles.js';
import { photoProPaletteCss, photoProTokenDeclarations } from './PhotoProThemeTokens.js';
import { photoProCardCss } from './PhotoProCardStyles.js';
import { SPINNER_WIDTH } from './ControlGeometry.js';
import { nativeFieldLayoutCss } from './NativeFieldLayoutStyles.js';
const regularFont = new URL('../../../../fonts/SourceSans3-Regular.ttf', import.meta.url).href;
const semiboldFont = new URL('../../../../fonts/SourceSans3-Semibold.ttf', import.meta.url).href;
/** Retain shared card, masonry, field and scrollbar geometry with medium-gray materials. */
export function photoProAppearanceCss() {
    return (nativeAppearanceCss({
        tokens: photoProTokenDeclarations(),
        canvas: 'var(--sc-photopro-pasteboard)',
        cardFill: 'var(--sc-surface)',
        cardStroke: 'var(--sc-border)',
        rowStroke: 'var(--sc-border-subtle)',
        controlFill: 'var(--sc-field-fill)',
        controlStroke: 'var(--sc-field-stroke)',
        controlBottomStroke: 'var(--sc-field-bottom-stroke)',
        radius: '4px',
    }, true) +
        nativeFieldLayoutCss('html[data-sc-visual-style="photopro"] :where([data-sc-style-node])') +
        photoProControlCss() +
        photoProComboCss() +
        photoProCardCss() +
        photoProPaletteCss());
}
/** Skin owned controls, menus and tabs without taking their interaction or layout owners. */
export function photoProControlCss() {
    const standaloneButton = 'html[data-sc-visual-style="photopro"] :is([data-sc-theme="photopro"], [data-sc-style-node], [data-sc-style-surface], [data-sc-style-popup]) :is(button,[data-sc-button]):where(:not([data-sc-style-combo-frame] > button,[data-sc-seed-split] > button,.sc-generation-strip button))';
    const standaloneField = 'html[data-sc-visual-style="photopro"] :is([data-sc-theme="photopro"], [data-sc-style-node], [data-sc-style-surface], [data-sc-style-popup]) :is(input:not([type="checkbox"]):not([type="range"]),textarea,[role="combobox"]):where(:not([data-sc-style-combo-frame] > [role="combobox"],[data-sc-number-input],.sc-generation-strip input))';
    return `
    html[data-sc-visual-style="photopro"] { --sc-control-height:22px; --sc-spinner-stepper-width:0px; }
    @font-face { font-family:"PhotoPro Source Sans 3"; src:url("${regularFont}") format("truetype"); font-weight:400; font-display:swap; }
    @font-face { font-family:"PhotoPro Source Sans 3"; src:url("${semiboldFont}") format("truetype"); font-weight:600; font-display:swap; }
    html[data-sc-visual-style="photopro"] :is([data-sc-theme="photopro"], [data-sc-style-node], [data-sc-style-surface], [data-sc-style-popup]) {
      color-scheme:dark; font-family:var(--sc-font-family); color:var(--sc-text);
    }
    html[data-sc-visual-style="photopro"][data-sc-photopro-palette="light-gray"] :is([data-sc-theme="photopro"], [data-sc-style-node], [data-sc-style-surface], [data-sc-style-popup]) { color-scheme:light; }
    ${standaloneButton} {
      color:var(--sc-text); background:var(--sc-button-fill); border:1px solid var(--sc-button-bottom-stroke);
      border-radius:var(--sc-button-radius); font:inherit;
    }
    ${standaloneButton}:hover:not(:disabled) { background:var(--sc-button-hover-fill); }
    ${standaloneButton}:active:not(:disabled) { background:var(--sc-button-pressed-fill); }
    ${standaloneField} {
      color:var(--sc-text); background:var(--sc-field-fill); border:1px solid var(--sc-field-stroke);
      border-radius:var(--sc-field-radius); font:inherit;
    }
    ${standaloneField}:hover { border-color:var(--sc-field-hover-stroke); }
    html[data-sc-visual-style="photopro"] :is([data-sc-theme="photopro"], [data-sc-style-node], [data-sc-style-surface], [data-sc-style-popup]) :is(input,textarea,button,[role="tab"],[role="menuitem"]):focus-visible { outline:1px solid var(--sc-focus-ring); outline-offset:1px; }
    html[data-sc-visual-style="photopro"] :is([data-sc-boolean-checkbox], .sc-overrides-toolbar[data-presentation="plugin"] > .sc-overrides-menu) { width:14px; height:14px; }
    html[data-sc-visual-style="photopro"] [data-sc-boolean-checkbox] { appearance:none; position:relative; box-sizing:border-box; margin:0; border:1px solid var(--sc-text-secondary); border-radius:3px; background:var(--sc-field-fill); flex:none; }
    html[data-sc-visual-style="photopro"] [data-sc-boolean-checkbox]:checked { background:var(--sc-text-secondary); border-color:var(--sc-text-secondary); }
    html[data-sc-visual-style="photopro"] [data-sc-boolean-checkbox]:checked::after { content:''; position:absolute; box-sizing:border-box; width:9px; height:6px; left:50%; top:50%; border-left:2px solid var(--sc-surface-secondary); border-bottom:2px solid var(--sc-surface-secondary); transform:translate(-50%,-65%) rotate(-45deg); }
    html[data-sc-visual-style="photopro"] [data-sc-boolean-checkbox]:disabled { background:var(--sc-surface-tertiary); border-color:var(--sc-border-disabled); }
    html[data-sc-visual-style="photopro"] [data-sc-number-pair] { gap:10px; }
    html[data-sc-visual-style="photopro"] [data-sc-style-row][data-sc-style-widget^="WidgetInputNumber"] [data-sc-style-layout] > [data-sc-number-pair] { flex:0 0 calc(${SPINNER_WIDTH} + 100px); max-width:calc(${SPINNER_WIDTH} + 100px); }
    html[data-sc-visual-style="photopro"] [data-sc-number-pair] > :first-child { order:1; align-self:center; }
    html[data-sc-visual-style="photopro"] [data-sc-number-input] { padding:0 4px; text-align:left; font:10px/12px var(--sc-font-family); }
    html[data-sc-visual-style="photopro"] :is([data-sc-number-increment],[data-sc-number-decrement]) { display:none; }
    html[data-sc-visual-style="photopro"] input[data-sc-number-slider] {
      --sc-photopro-range-thumb-width:11px;
      appearance:none; -webkit-appearance:none; position:relative; top:-4px; order:0; height:23px; margin:0 0 0 10px; padding:0;
      border:0; background:transparent; cursor:default; flex:1 1 auto; max-width:none;
    }
    html[data-sc-visual-style="photopro"] input[data-sc-number-slider]::-webkit-slider-runnable-track {
      height:3px; border:0; border-radius:0;
      background:linear-gradient(var(--sc-photopro-range-track),var(--sc-photopro-range-track)) center / calc(100% - var(--sc-photopro-range-thumb-width)) 100% no-repeat;
    }
    html[data-sc-visual-style="photopro"] input[data-sc-number-slider]::-moz-range-track {
      height:3px; border:0; border-radius:0;
      background:linear-gradient(var(--sc-photopro-range-track),var(--sc-photopro-range-track)) center / calc(100% - var(--sc-photopro-range-thumb-width)) 100% no-repeat;
    }
    html[data-sc-visual-style="photopro"] input[data-sc-number-slider]::-moz-range-progress {
      height:3px; border:0; background:transparent;
    }
    html[data-sc-visual-style="photopro"] input[data-sc-number-slider]::-webkit-slider-thumb {
      appearance:none; -webkit-appearance:none; box-sizing:border-box; width:var(--sc-photopro-range-thumb-width); height:10px; margin-top:2px;
      border:1px solid var(--sc-border); border-radius:0; background:var(--sc-photopro-range-thumb);
      clip-path:polygon(50% 0, 100% 62%, 100% 85%, 90% 100%, 10% 100%, 0 85%, 0 62%);
    }
    html[data-sc-visual-style="photopro"] input[data-sc-number-slider]::-moz-range-thumb {
      appearance:none; box-sizing:border-box; width:var(--sc-photopro-range-thumb-width); height:10px; transform:translateY(5px);
      border:1px solid var(--sc-border); border-radius:0; background:var(--sc-photopro-range-thumb);
      clip-path:polygon(50% 0, 100% 62%, 100% 85%, 90% 100%, 10% 100%, 0 85%, 0 62%);
    }
    html[data-sc-visual-style="photopro"] input[data-sc-number-slider]:active::-webkit-slider-thumb { background:var(--sc-text); }
    html[data-sc-visual-style="photopro"] input[data-sc-number-slider]:active::-moz-range-thumb { background:var(--sc-text); }
    html[data-sc-visual-style="photopro"] input[data-sc-number-slider]:disabled { opacity:.45; cursor:default; }
    html[data-sc-visual-style="photopro"] .sc-photopro-editor-field { width:100%; height:100%; min-width:0; min-height:0; box-sizing:border-box; border:1px solid var(--sc-field-stroke); border-radius:2px; background:var(--sc-field-fill); overflow-x:hidden; overflow-y:auto; --sc-editor-content-padding:4px 6px; --sc-editor-content-padding-left:6px; }
    html[data-sc-visual-style="photopro"] .sc-photopro-editor-field:focus-within { border-color:var(--sc-focus-ring); }
    html[data-sc-visual-style="photopro"] sc-photopro-search { display:block; min-width:0; }
    html[data-sc-visual-style="photopro"] sc-photopro-search input { box-sizing:border-box; width:100%; min-width:0; height:30px; padding:4px 8px; }
    html[data-sc-visual-style="photopro"] [data-sc-boolean-segments] { background:var(--sc-surface-secondary); border-color:var(--sc-border); border-radius:3px; }
    html[data-sc-visual-style="photopro"] [data-sc-boolean-segment] { border:0; border-radius:2px; background:transparent; }
    html[data-sc-visual-style="photopro"] [data-sc-boolean-segment][aria-selected="true"] { background:var(--sc-surface-tertiary); color:var(--sc-text); }
    html[data-sc-visual-style="photopro"] [data-sc-workflow-tabs] { display:flex; height:30px; width:var(--sc-workflow-tabs-width); min-width:0; overflow:clip; background:var(--sc-photopro-tab); }
    html[data-sc-visual-style="photopro"] [data-sc-workflow-tab] { display:inline-flex; align-items:center; box-sizing:border-box; height:30px; width:var(--sc-workflow-tab-width,180px); min-width:var(--sc-workflow-tab-width,180px); max-width:var(--sc-workflow-tab-width,180px); flex:0 0 var(--sc-workflow-tab-width,180px); padding:0 8px 0 20px; gap:6px; border:0; border-right:1px solid var(--sc-border); border-radius:0; background:var(--sc-photopro-tab); color:var(--sc-text-subtle); font:600 12px/1.3 var(--sc-font-family); }
    html[data-sc-visual-style="photopro"] [data-sc-workflow-tab][aria-selected="true"] { background:var(--sc-surface); color:var(--sc-text); }
    html[data-sc-visual-style="photopro"] [data-sc-workflow-tab][data-orb-adjacent] { padding-left:32px; }
    html[data-sc-visual-style="photopro"] [data-sc-workflow-tab] .sc-workflow-title { flex:1; min-width:0; overflow:hidden; text-overflow:ellipsis; white-space:nowrap; }
    html[data-sc-visual-style="photopro"] .sc-substitute-frame[data-sc-theme="photopro"] { background:var(--sc-surface-secondary); --sc-frame-muted:var(--sc-text-subtle); --sc-frame-hover:var(--sc-surface-hover); --sc-frame-pressed:var(--sc-surface-pressed); }
  `;
}
