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
/** Match the actual desktop cart geometry with scoped Fluent and Comfy appearance rules. */
import { ensureCubeStackStyles } from '../surface/stack/CubeStackStyles.js';
/** Install trusted static dialog and pane rules once without affecting unrelated native surfaces. */
export function ensureCubeStackCartStyles(document) {
    ensureCubeStackStyles(document);
    if (document.getElementById('sc-cube-stack-cart-styles'))
        return;
    const style = document.createElement('style');
    style.id = 'sc-cube-stack-cart-styles';
    style.textContent = `
    .sugarcubes-modal.sc-cube-stack-cart-dialog { width:528px; max-width:calc(100vw - 32px); height:653px; max-height:88vh; display:flex; flex-direction:column; overflow:hidden; }
    .sc-cube-stack-cart-dialog > .p-dialog-header { flex:0 0 auto; }
    .sugarcubes-modal.sc-cube-stack-cart-dialog > .p-dialog-content { flex:1 1 auto; min-height:0; overflow:hidden; display:flex; flex-direction:column; padding:0 24px 20px; }
    .sc-cube-stack-cart-dialog > .p-dialog-footer { flex:0 0 auto; }
    .sc-cube-stack-cart { display:grid; grid-template-columns:minmax(0,1fr) minmax(0,1fr); grid-template-rows:auto minmax(0,1fr); gap:8px; width:100%; min-height:0; height:100%; flex:1; }
    .sc-cube-stack-cart__header { grid-column:1 / -1; }
    .sc-cube-stack-cart__library-label { font-size:14px; margin:0 0 5px; }
    .sugarcubes-modal.sc-cube-stack-cart-dialog .p-dialog-title { font-size:28px; line-height:36px; font-weight:400; }
    .sc-cube-stack-cart .sc-cube-stack__rows { box-sizing:border-box; width:224px; max-width:100%; }
    .sc-cube-stack-cart [aria-label="Cube library"] .sc-cube-stack__rows { width:212px; padding:0; }
    .sc-cube-stack-cart__pane { width:100%; min-width:0; min-height:0; display:flex; flex-direction:column; }
    .sc-cube-stack-cart__search { box-sizing:border-box; width:100%; max-width:none; min-height:32px; margin:0; flex:0 0 auto; color:var(--sc-stack-text); }
    input.sc-cube-stack-cart__search { border:1px solid var(--sc-stack-border); background:var(--sc-stack-row); border-radius:5px; padding:6px 10px; font:inherit; }
    .sc-cube-stack-cart__scroll { overflow:auto; min-height:0; flex:1; scrollbar-width:thin; }
    .sc-cube-stack-cart__heading { font:400 14px/20px var(--fontFamilyBase,"Segoe UI",sans-serif); margin:0 6px 4px; min-height:20px; }
    .sc-cube-stack-cart__model { font:600 14px/28px var(--fontFamilyBase,"Segoe UI",sans-serif); margin:0 6px; }
    .sc-cube-stack-cart__role { font:400 10px/18px var(--fontFamilyBase,"Segoe UI",sans-serif); color:var(--sc-stack-muted); margin:0 6px 6px; }
    .sc-cube-stack-cart__scroll section + section { margin-top:6px; }
    .sc-cube-stack-cart__empty { min-height:64px; display:flex; align-items:center; justify-content:center; text-align:center; color:var(--sc-stack-muted); font-size:12px; margin:8px 6px; }
    .sc-cube-stack-cart [aria-label="Cube cart"] .sc-cube-stack-row__remove { opacity:1; }
    .sc-cube-stack-cart__drop { border-radius:5px; }
    .sc-cube-stack-cart__drop:empty { border:1px dashed var(--sc-stack-border); }
    .sc-cube-stack-cart-dialog .sc-cube-stack-cart-refresh { margin-left:auto; margin-right:8px; }
    .sc-cube-stack-cart-reset { margin-right:auto; }
    .sc-cube-stack-cart-dialog .sugarcubes-modal__footer-actions { width:100%; justify-content:flex-end; }
    @media (max-width:480px) { .sugarcubes-modal.sc-cube-stack-cart-dialog > .p-dialog-content { padding-left:12px; padding-right:12px; } }
  `;
    document.head.append(style);
}
