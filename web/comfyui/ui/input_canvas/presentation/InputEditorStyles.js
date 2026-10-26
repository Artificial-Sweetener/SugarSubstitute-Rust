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
/** Present selected-large and compact-related source previews in masonry. */
export function ensureInputEditorStyles(document) {
    if (document.getElementById('sc-input-editor-styles'))
        return;
    const style = document.createElement('style');
    style.id = 'sc-input-editor-styles';
    style.textContent = `
  .sc-input-editor__field {font:inherit;font-weight:600;margin:0 0 4px;}
  .sc-input-editor {display:flex;flex-direction:column;gap:6px;padding:6px 8px;color:var(--fg-color);font-size:12px;min-width:0;}
  :is(.sc-input-editor,[data-sc-input-controls]) [hidden] {display:none!important;}
  .sc-input-editor__controls {border:0;margin:0;padding:6px 8px;min-width:0;display:flex;flex-direction:column;gap:6px;font:inherit;color:inherit;}
  .sc-canvas-source {display:flex;flex-direction:column;gap:6px;min-width:0;}
  [data-input-empty][hidden] {display:none!important;}
  .sc-input-editor__rows {display:flex;flex-direction:column;gap:4px;}
  .sc-input-editor__row {display:grid;grid-template-columns:auto 1fr;align-items:center;gap:8px;padding:6px 8px 6px 10px;border:0;border-left:4px solid var(--sc-mask-color,var(--p-primary-color,currentColor));background:transparent;color:inherit;cursor:pointer;min-height:44px;text-align:left;}
  .sc-input-editor__row[data-passive=false]:hover {background:color-mix(in srgb,currentColor 9%,transparent);}
  .sc-input-editor__row[data-selected=true] {grid-template-columns:1fr;background:color-mix(in srgb,currentColor 13%,transparent);}
  .sc-input-editor__row[data-selected=true] .sc-input-editor__label {grid-row:1;}
  .sc-input-editor__frame {position:relative;width:44px;max-width:100%;justify-self:center;overflow:hidden;background:#000;border-radius:6px;}
  .sc-input-editor__preview {position:absolute;inset:0;display:block;width:100%;height:100%;object-fit:contain;}
  .sc-input-editor__crop {position:absolute;outline:1px solid red;pointer-events:none;}
  .sc-input-editor__row[data-selected=true] .sc-input-editor__frame {width:288px;}
  .sc-input-editor__row[data-passive=true] {border:0;padding:0;cursor:default;background:transparent;}
  .sc-input-editor__row[data-passive=true] .sc-input-editor__frame {border-radius:0;}
  .sc-input-editor__placeholder {position:absolute;inset:0;display:flex;flex-direction:column;align-items:center;justify-content:center;gap:6px;padding:8px;text-align:center;}
  .sc-input-editor__placeholder-icon {font-size:22px;opacity:.7;}
  .sc-input-editor__actions {display:flex;flex-wrap:wrap;gap:4px;}
  .sc-input-editor__row:disabled {opacity:.45;cursor:default;}
  .sc-input-editor__row:focus-visible {outline:1px solid currentColor;outline-offset:1px;}
  .sc-input-editor__status {font-size:11px;opacity:.8;min-height:1em;}
  .sc-input-editor__status:empty,.sc-input-editor__message:empty {display:none;}
  `;
    document.head.append(style);
}
