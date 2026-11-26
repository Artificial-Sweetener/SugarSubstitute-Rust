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
/** The full-window file editor's layout: a file list beside the editor, 7 to 18 as in Python. */
const installed = new WeakSet();
const CSS = `
.sc-text-assets {
  flex: 1 1 auto;
  min-height: 0;
  display: flex;
  flex-direction: column;
  gap: 12px;
}
.sc-text-assets__toolbar { display: flex; gap: 8px; flex-wrap: wrap; }
.sc-text-assets__create { display: inline-flex; align-items: center; gap: 6px; }
.sc-text-assets__panes {
  flex: 1 1 auto;
  min-height: 0;
  display: grid;
  grid-template-columns: minmax(220px, 7fr) minmax(0, 18fr);
  gap: 12px;
}
.sc-text-assets__card {
  min-height: 0;
  display: flex;
  flex-direction: column;
  gap: 8px;
  padding: 12px;
  border-radius: 8px;
  border: 1px solid light-dark(#0000001a, #ffffff14);
  background: light-dark(#00000005, #ffffff06);
}
.sc-text-assets__title {
  margin: 0;
  font-size: 14px;
  font-weight: 700;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.sc-text-assets__file .sc-text-assets__title { font-size: 16px; flex: 1 1 auto; }
.sc-text-assets__list { flex: 1 1 auto; min-height: 0; overflow-y: auto; overflow-x: hidden; }
.sc-text-assets__empty { margin: 4px 0; opacity: 0.7; }
.sc-text-assets__group {
  margin: 10px 0 4px;
  font-size: 12px;
  font-weight: 700;
  opacity: 0.65;
}
.sc-text-assets__group:first-child { margin-top: 0; }
.sc-text-assets__row {
  all: unset;
  box-sizing: border-box;
  display: flex;
  flex-direction: column;
  gap: 2px;
  width: 100%;
  min-height: 56px;
  padding: 8px 10px;
  border-radius: 6px;
  border-left: 3px solid transparent;
  cursor: pointer;
}
.sc-text-assets__row.is-alternate { background: light-dark(#00000005, #ffffff05); }
.sc-text-assets__row:hover { background: light-dark(#0000000c, #ffffff0c); }
.sc-text-assets__row[aria-selected="true"] {
  background: light-dark(#00000011, #ffffff11);
  border-left-color: var(--sc-accent-indicator, var(--sc-brand-border, Highlight));
}
.sc-text-assets__row[aria-selected="true"]:hover { background: light-dark(#00000019, #ffffff19); }
.sc-text-assets__row:focus-visible { outline: 2px solid var(--sc-accent-indicator, Highlight); }
.sc-text-assets__row.is-disabled .sc-text-assets__label { opacity: 0.6; }
.sc-text-assets__label,
.sc-text-assets__subtitle { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.sc-text-assets__label { font-weight: 700; }
.sc-text-assets__subtitle { font-size: 12px; opacity: 0.7; }
.sc-text-assets__editor-head { display: flex; align-items: center; gap: 4px; }
.sc-text-assets__tool { display: inline-flex; align-items: center; justify-content: center; }
.sc-text-assets__editor { flex: 1 1 auto; min-height: 180px; position: relative; }
`;
/** Install the editor's rules once per document. */
export function installTextAssetStyles(document) {
    if (installed.has(document))
        return;
    const style = document.createElement('style');
    style.dataset.scTextAssetStyles = '';
    style.textContent = CSS;
    document.head.append(style);
    installed.add(document);
}
