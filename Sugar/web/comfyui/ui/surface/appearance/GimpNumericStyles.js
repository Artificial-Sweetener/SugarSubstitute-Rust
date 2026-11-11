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
/** Keep GIMP numeric entries and native step buttons in one shared grid row. */
import { CONTROL_HEIGHT, SPINNER_WIDTH, NUMERIC_PAIR_GAP, NUMERIC_PAIR_WIDTH, } from './ControlGeometry.js';
const addIcon = new URL('../../../../icons/node/gimp/list-add-symbolic.svg', import.meta.url).href;
const removeIcon = new URL('../../../../icons/node/gimp/list-remove-symbolic.svg', import.meta.url)
    .href;
/** Give the numeric group sole height ownership while GTK-shaped children stretch inside it. */
export function gimpNumericCss() {
    const scope = 'html[data-sc-visual-style="gimp"]';
    const spinner = `${scope} [data-sc-number-spinner]:not([data-sc-seed-box])`;
    const pair = `${scope} [data-sc-number-pair]:has(> sc-gimp-scale)`;
    const scaleSpinner = `${pair} [data-sc-number-spinner]:not([data-sc-seed-box])`;
    const sliderPair = `${scope} [data-sc-number-pair]:has(> sc-gimp-slider)`;
    return `
    ${spinner} { display:grid; grid-template-columns:minmax(0,1fr) repeat(2,var(--sc-number-stepper-width)); grid-template-rows:minmax(0,1fr); align-items:stretch; padding:0; border:0; border-radius:0; background:transparent; box-shadow:none; overflow:visible; }
    ${scope} [data-sc-style-layout] > :last-child > :first-child:has([data-sc-number-spinner]:not([data-sc-seed-box])) { margin:0; padding:0; border:0; border-radius:0; background:transparent; box-shadow:none; }
    ${spinner} > :has(> [data-sc-number-increment]), ${spinner} > :has(> [data-sc-number-decrement]) { display:contents; }
    ${spinner} > div:has(> [data-sc-number-input]), ${spinner} > [data-sc-number-input] { box-sizing:border-box; grid-column:1; grid-row:1; align-self:stretch; height:auto; min-width:0; min-height:0; margin:0; padding:0; border:1px solid var(--sc-gimp-strongBorder); border-right:0; border-radius:3px 0 0 3px; background:var(--sc-gimp-widget); box-shadow:none; }
    ${spinner} [data-sc-number-input] { text-align:left; padding:0 6px; }
    ${spinner} :is([data-sc-number-increment],[data-sc-number-decrement]) { position:static; inset:auto; box-sizing:border-box; grid-row:1; align-self:stretch; justify-self:stretch; aspect-ratio:auto; width:auto; height:auto; min-width:0; min-height:0; padding:0; margin:0; border:1px solid var(--sc-gimp-strongBorder); border-radius:0; background:var(--sc-gimp-background); color:var(--sc-gimp-dimmed); box-shadow:none; }
    ${spinner} [data-sc-number-decrement] { grid-column:2; }
    ${spinner} [data-sc-number-increment] { grid-column:3; border-left:0; border-radius:0 3px 3px 0; }
    ${spinner} :is([data-sc-number-increment],[data-sc-number-decrement]) :is(i,span,svg) { display:none; }
    ${spinner} :is([data-sc-number-increment],[data-sc-number-decrement])::before { content:''; width:16px; height:16px; flex:none; background:currentColor; mask-size:contain; mask-repeat:no-repeat; mask-position:center; }
    ${spinner} [data-sc-number-increment]::before { mask-image:url('${addIcon}'); }
    ${spinner} [data-sc-number-decrement]::before { mask-image:url('${removeIcon}'); }
    ${spinner} :is([data-sc-number-increment],[data-sc-number-decrement]):hover:not(:disabled) { background:var(--sc-gimp-selected); color:var(--sc-gimp-foreground); }
    ${pair} { position:relative; display:grid; grid-template-columns:minmax(0,1fr); grid-template-rows:minmax(0,1fr); align-items:stretch; flex:1 1 100%; width:100%; max-width:100%; height:${CONTROL_HEIGHT}; gap:0; }
    ${scope} [data-sc-style-row][data-sc-style-widget^="WidgetInputNumber"] [data-sc-style-layout] > [data-sc-number-pair]:has(> sc-gimp-scale) { flex:1 1 100%; max-width:100%; }
    ${scope} [data-sc-style-layout]:has(> [data-sc-number-pair] > sc-gimp-scale) > :first-child { display:none; }
    ${pair} > :first-child { position:relative; z-index:1; justify-self:end; height:100%; margin:0; padding:0; border:0; border-radius:0; background:transparent; box-shadow:none; }
    ${pair} [data-sc-number-spinner] { height:100%; }
    ${scaleSpinner} > div:has(> [data-sc-number-input]), ${scaleSpinner} > [data-sc-number-input] { border:0; background:transparent; }
    ${scaleSpinner} [data-sc-number-input] { text-align:right; }
    ${pair} > sc-gimp-scale { position:absolute; inset:0 var(--sc-spinner-stepper-width) 0 0; width:calc(100% - var(--sc-spinner-stepper-width)); height:100%; min-width:0; max-width:none; }
    ${pair} [data-sc-number-increment] { border-radius:0 5px 5px 0; }
    ${pair} :is([data-sc-number-increment],[data-sc-number-decrement]) { border-color:var(--sc-gimp-border); box-shadow:0 .5px var(--sc-gimp-strongerBorder); }
    ${sliderPair} { display:grid; grid-template-columns:minmax(40px,1fr) ${SPINNER_WIDTH}; grid-template-rows:minmax(0,1fr); align-items:stretch; height:${CONTROL_HEIGHT}; gap:${NUMERIC_PAIR_GAP}; }
    ${scope} [data-sc-style-row][data-sc-style-widget^="WidgetInputNumber"] [data-sc-style-layout] > [data-sc-number-pair]:has(> sc-gimp-slider) { flex:0 1 ${NUMERIC_PAIR_WIDTH}; max-width:${NUMERIC_PAIR_WIDTH}; }
    ${scope} [data-sc-style-layout]:has(> [data-sc-number-pair] > sc-gimp-slider) > :first-child { flex:0 0 auto; max-width:35%; }
    ${sliderPair} > :first-child { grid-column:2; grid-row:1; height:100%; }
    ${sliderPair} [data-sc-number-spinner] { height:100%; }
    ${sliderPair} > sc-gimp-slider { grid-column:1; grid-row:1; width:100%; height:100%; min-width:40px; max-width:none; }
  `;
}
