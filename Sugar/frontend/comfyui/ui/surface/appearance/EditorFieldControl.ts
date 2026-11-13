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
/** Let the selected input provider own field chrome while an external editor owns text and history. */
export interface EditorFieldControl {
  readonly root: HTMLElement;
  /** Attach the sole editable surface after the provider root is connected. */
  attachEditor(editor: HTMLElement): void;
  /** Reflect editing permission without introducing another value owner. */
  setReadOnly(readOnly: boolean): void;
}

/** Transfer native input and frame spacing into the editor so its viewport reaches the field border. */
export function adoptEditorFieldPadding(field: HTMLElement, nativeInput: HTMLElement): void {
  const window = field.ownerDocument.defaultView;
  const frame = nativeInput === field ? field : nativeInput.parentElement;
  if (!window || !frame) throw new Error('The editor field requires a connected native frame.');
  // Re-read the provider's stylesheet after size/validation changes, excluding our previous transfer.
  frame.style.removeProperty('padding');
  const input = window.getComputedStyle(nativeInput);
  const outer = frame === nativeInput ? null : window.getComputedStyle(frame);
  /** Retain provider-defined spacing as content padding inside the sole scrolling surface. */
  const padding = (side: 'paddingTop' | 'paddingRight' | 'paddingBottom' | 'paddingLeft'): string =>
    `calc(${input[side] || '0px'} + ${outer?.[side] || '0px'})`;
  field.style.setProperty(
    '--sc-editor-content-padding',
    `${padding('paddingTop')} ${padding('paddingRight')} ${padding('paddingBottom')} ${padding('paddingLeft')}`,
  );
  field.style.setProperty('--sc-editor-content-padding-left', padding('paddingLeft'));
  frame.style.padding = '0';
}
