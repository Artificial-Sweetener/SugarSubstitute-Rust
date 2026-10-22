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
/** Style prompt decorations while shared provider controls own field presentation. */
const installed = new WeakSet<Document>();
/** Install editor-only decoration rules without recreating input chrome. */
export function installSugarPromptStyles(document: Document): void {
  if (installed.has(document)) return;
  const style = document.createElement('style');
  style.dataset.scPromptStyles = '';
  style.textContent = `
.lg-node-widget :has(> .sc-cube-prompt-editor) > label {
  display: none;
}
.sugarprompt-editor {
  min-height: 0;
  position: relative;
  box-sizing: border-box;
}
.sugarprompt-editor .cm-editor.cm-focused { outline: none; }
.sugarprompt-editor .cm-content:focus { outline: none; }
.sugarprompt-editor .cm-content {
  padding:var(--sc-editor-content-padding);
  padding-left:calc(var(--sc-editor-content-padding-left) + var(--sugarprompt-content-inset,0px));
}
.sugarprompt-editor :is(.cm-editor[data-reorder-preview="true"],.sp-reorder-preview) .sp-reorder-chip {
  outline:1px solid var(--sc-accent-indicator,var(--sc-brand-border,Highlight));
  background-color:color-mix(in srgb,var(--sc-accent-indicator,var(--sc-brand-border,Highlight)) 12%,transparent);
  border-radius:9px; box-decoration-break:clone; -webkit-box-decoration-break:clone;
}
.sugarprompt-editor :is(.cm-editor[data-reorder-preview="true"],.sp-reorder-preview) .sp-reorder-chip:is(:hover,.sp-reorder-chip-active) {
  background-color:color-mix(in srgb,var(--sc-accent-indicator,var(--sc-brand-border,Highlight)) 22%,transparent);
}
.sugarprompt-editor .sp-reorder-empty { border:none; outline:none; background:transparent; }
`;
  document.head.append(style);
  installed.add(document);
}
