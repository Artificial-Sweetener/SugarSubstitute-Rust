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
/** Translate GIMP's common.css and System/gimp.css control paint into shadow-scoped browser CSS. */
import { CONTROL_HEIGHT } from './ControlGeometry.js';
/** Keep all browser-native input parts on the same inherited GIMP token contract. */
export const gimpControlStyles = `
  :host { display:inline-flex; box-sizing:border-box; min-width:0; color:var(--sc-text); font:var(--sc-font-size,12px)/var(--sc-line-height,18px) var(--sc-font-family,sans-serif); }
  :host(sc-gimp-scale) { height:${CONTROL_HEIGHT}; min-width:100px; }
  :host(sc-gimp-slider) { align-items:center; height:${CONTROL_HEIGHT}; min-width:40px; }
  *,*::before,*::after { box-sizing:border-box; }
  button,input { font:inherit; color:inherit; min-width:0; margin:0; }
  button { display:inline-flex; align-items:center; justify-content:center; gap:4px; width:100%; height:100%; padding:3px 6px; border:1px solid var(--sc-gimp-strongBorder); border-radius:3px; background:var(--sc-gimp-widget); box-shadow:inset 0 1px var(--sc-gimp-extremeSelected); cursor:default; }
  button:hover:not(:disabled) { background:var(--sc-gimp-hover); box-shadow:none; }
  button:active:not(:disabled),:host([aria-selected="true"]) button,:host([aria-pressed="true"]) button { background:var(--sc-gimp-selected); box-shadow:none; }
  :host([disabled]) { color:var(--sc-text-disabled); cursor:default; }
  :host([tabindex="-1"]) button { outline:none; }
  button:focus-visible,input:focus-visible,:host([role="tab"]:focus-visible) { outline:1px dotted var(--sc-gimp-foreground); outline-offset:-3px; }
  input[type=search] { width:100%; height:100%; border:1px solid var(--sc-gimp-strongerBorder); border-radius:3px; background:var(--sc-gimp-widget); padding:3px 6px; }
  input[type=checkbox] { appearance:none; width:14px; height:14px; border:1px solid var(--sc-gimp-strongerBorder); border-radius:3px; background:var(--sc-gimp-widget); position:relative; flex:none; }
  input[type=checkbox]:hover:not(:disabled) { border-color:var(--sc-gimp-foreground); }
  input[type=checkbox]:checked::after { content:''; position:absolute; width:9px; height:6px; top:50%; left:50%; border-left:2px solid currentColor; border-bottom:2px solid currentColor; transform:translate(-50%,-50%) rotate(-45deg); }
  input:disabled { color:var(--sc-text-disabled); opacity:.65; }
  .scale-slider { appearance:none; width:100%; height:${CONTROL_HEIGHT}; padding:0; border:0; background:transparent; cursor:default; }
  .scale-slider::-webkit-slider-runnable-track { box-sizing:border-box; height:6px; border:1px solid var(--sc-gimp-strongBorder); border-radius:3px; background:linear-gradient(to right,var(--sc-gimp-scrollbar) var(--sc-gimp-slider-fraction),var(--sc-gimp-ruler) var(--sc-gimp-slider-fraction)); }
  .scale-slider::-webkit-slider-thumb { appearance:none; box-sizing:border-box; width:18px; height:18px; margin-top:-7px; border:1px solid var(--sc-gimp-strongBorder); border-radius:50%; background:var(--sc-gimp-widget); box-shadow:inset 0 1px var(--sc-gimp-extremeSelected); }
  .scale-slider::-moz-range-track { box-sizing:border-box; height:6px; border:1px solid var(--sc-gimp-strongBorder); border-radius:3px; background:var(--sc-gimp-ruler); }
  .scale-slider::-moz-range-progress { height:4px; border:0; border-radius:3px; background:var(--sc-gimp-scrollbar); }
  .scale-slider::-moz-range-thumb { box-sizing:border-box; width:18px; height:18px; border:1px solid var(--sc-gimp-strongBorder); border-radius:50%; background:var(--sc-gimp-widget); box-shadow:inset 0 1px var(--sc-gimp-extremeSelected); }
  .scale-slider:hover::-webkit-slider-thumb { background:var(--sc-gimp-hover); }
  .scale-slider:hover::-moz-range-thumb { background:var(--sc-gimp-hover); }
  .spin-scale { position:relative; width:100%; min-width:0; height:100%; border:1px solid var(--sc-gimp-edgeBorder); border-right:0; border-radius:5px 0 0 5px; background:var(--sc-gimp-spin-scale-entry); overflow:hidden; }
  .spin-scale-progress { position:absolute; inset:0 auto 0 0; width:calc(var(--sc-gimp-scale-fraction,0) * 100%); border:0; border-radius:3px 0 0 3px; background:var(--sc-gimp-spin-scale-fill); background-image:none; pointer-events:none; }
  :host(:dir(rtl)) .spin-scale-progress { left:auto; right:0; border-radius:0 3px 3px 0; }
  .spin-scale-label { position:absolute; inset:0 calc(5ch + 18px) 0 6px; display:flex; align-items:center; overflow:hidden; white-space:nowrap; pointer-events:none; }
  .spin-scale:focus-within { outline:1px dotted var(--sc-gimp-foreground); outline-offset:-3px; }
  .spin-scale input[type=range] { appearance:none; position:absolute; inset:0; width:100%; height:100%; padding:0; opacity:0; cursor:default; }
  .spin-scale input[type=range]::-webkit-slider-runnable-track { height:100%; border:0; }
  .spin-scale input[type=range]::-moz-range-track { height:100%; border:0; }
  .spin-scale input[type=range]::-webkit-slider-thumb { appearance:none; width:1px; height:100%; border:0; }
  .spin-scale input[type=range]::-moz-range-thumb { width:1px; height:100%; border:0; }
  .frame { display:flex; align-items:center; width:100%; height:100%; min-width:0; min-height:0; }
  :host([data-sc-editor-field]) .frame { display:block; border:1px solid var(--sc-field-stroke); border-radius:3px; background:var(--sc-field-fill); }
  :host([data-sc-editor-field]):focus-within .frame { outline:1px dotted var(--sc-gimp-foreground); outline-offset:-3px; }
  ::slotted(.sc-workflow-title) { flex:1; min-width:0; }
`;
