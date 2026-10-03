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
/** Render owned activation switches independently of Comfy's lazy component styles. */
export function ensureCubeFaceActivationStyles(documentRef: Document): void {
  const styleId = 'sugarcubes-cube-activation-styles';
  if (documentRef.getElementById(styleId)) return;
  const style = documentRef.createElement('style');
  style.id = styleId;
  style.textContent = `
    [data-cube-card-activation] .p-toggleswitch {
      position:relative; display:inline-block; flex:0 0 auto;
      box-sizing:border-box; width:40px; height:20px; padding:0; margin:0; border:0;
      vertical-align:middle;
    }
    [data-cube-card-activation] .p-toggleswitch-input {
      position:absolute; inset:0; z-index:1; width:100%; height:100%;
      padding:0; margin:0; opacity:0; cursor:inherit;
    }
    [data-cube-card-activation] .p-toggleswitch-slider {
      box-sizing:border-box; position:absolute; inset:0; display:block;
      border:1px solid var(--colorNeutralForeground3, var(--p-text-muted-color, #999));
      border-radius:999px; background:transparent; pointer-events:none;
      transition:background-color var(--durationNormal, 200ms) var(--curveDecelerateMid, ease-out),
        border-color var(--durationNormal, 200ms) var(--curveDecelerateMid, ease-out);
    }
    [data-cube-card-activation] .p-toggleswitch-handle {
      position:absolute; top:3px; left:3px; display:block; width:12px; height:12px;
      box-sizing:border-box; margin:0; padding:0; border:0;
      border-radius:50%; background:var(--colorNeutralForeground2, var(--p-text-color, #ddd));
      transform:translateX(0);
      transition:transform var(--durationNormal, 200ms) var(--curveDecelerateMid, ease-out),
        background-color var(--durationNormal, 200ms) var(--curveDecelerateMid, ease-out);
    }
    [data-cube-card-activation] input:checked + .p-toggleswitch-slider {
      border-color:transparent;
      background:var(--colorCompoundBrandBackground, var(--p-primary-color, #60a5fa));
    }
    [data-cube-card-activation] input:checked + .p-toggleswitch-slider .p-toggleswitch-handle {
      transform:translateX(20px);
      background:var(--colorNeutralForegroundOnBrand, var(--p-primary-contrast-color, #111));
    }
    [data-cube-card-activation] input:checked:hover + .p-toggleswitch-slider {
      background:var(--colorCompoundBrandBackgroundHover, var(--p-primary-hover-color, #3b82f6));
    }
    [data-cube-card-activation] input:focus-visible + .p-toggleswitch-slider {
      outline:2px solid var(--colorStrokeFocus2, var(--p-primary-color, #60a5fa)); outline-offset:2px;
    }
    [data-cube-card-activation] input:disabled { cursor:default; }
    [data-cube-card-activation] input:disabled + .p-toggleswitch-slider { opacity:.45; }
    @media (prefers-reduced-motion:reduce) {
      [data-cube-card-activation] .p-toggleswitch-slider,
      [data-cube-card-activation] .p-toggleswitch-handle { transition:none; }
    }
  `;
  documentRef.head.append(style);
}
