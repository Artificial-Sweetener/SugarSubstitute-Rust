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
/** Completion alone reserves its adopted gap inside native tooltip space; CodeMirror retains side/anchor placement. */
import { requireValue } from '../../core/src/required.js';
import type { EditorView, Rect } from '@codemirror/view';
/** Native available height is expressed in local CSS pixels, including the popup's border box. */
export interface CompletionHeight {
  readonly outer: number;
  readonly inner: number;
  readonly visible: boolean;
}
/** Clamp the actual chosen side after reserving the adopted gap and native border/padding insets. */
export function completionHeight(
  space: Rect,
  caret: Rect,
  above: boolean,
  scale: number,
  border: number,
  lineHeight: number,
): CompletionHeight | null {
  const available = (above ? caret.top - space.top : space.bottom - caret.bottom) - 6;
  if (
    ![available, scale, border, lineHeight].every(Number.isFinite) ||
    scale <= 0 ||
    border < 0 ||
    lineHeight <= 0
  )
    return null;
  const outer = Math.max(0, available / scale);
  const inner = Math.max(0, outer - border);
  return { outer, inner, visible: inner >= lineHeight };
}
/** Read native operands at the CodeMirror logical-document UTF-16 caret offset; unavailable geometry never receives invented replacement coordinates. */
export function readCompletionHeight(
  view: EditorView,
  dom: HTMLElement,
  space: Rect | null,
  caret: number,
): CompletionHeight | null {
  if (!space) return null;
  const coordinates = view.coordsAtPos(caret);
  if (!coordinates) return null;
  const style = requireValue(dom.ownerDocument.defaultView, 'completion-space').getComputedStyle(
    dom,
  );
  const scale = dom.getBoundingClientRect().height / parseFloat(style.height);
  return completionHeight(
    space,
    coordinates,
    dom.classList.contains('cm-tooltip-above'),
    scale,
    parseFloat(style.borderTopWidth) +
      parseFloat(style.borderBottomWidth) +
      parseFloat(style.paddingTop) +
      parseFloat(style.paddingBottom),
    view.defaultLineHeight,
  );
}
/** Publish only the two owned height variables; callback reentry cannot continue an old geometry lease. */
export function applyCompletionHeight(
  dom: HTMLElement,
  height: CompletionHeight | null,
  current: () => boolean,
): 'changed' | 'unchanged' | 'retired' {
  const outer = height ? `${height.outer}px` : '',
    inner = height ? `${height.inner}px` : '',
    visibility = height?.visible ? '' : 'hidden';
  if (
    dom.style.getPropertyValue('--sp-completion-outer-height') === outer &&
    dom.style.getPropertyValue('--sp-completion-inner-height') === inner &&
    dom.style.visibility === visibility
  )
    return current() ? 'unchanged' : 'retired';
  if (!current()) return 'retired';
  if (visibility) {
    dom.style.visibility = visibility;
    if (!current()) return 'retired';
  }
  dom.style.setProperty('--sp-completion-outer-height', outer);
  if (!current()) return 'retired';
  dom.style.setProperty('--sp-completion-inner-height', inner);
  if (!current()) return 'retired';
  dom.style.visibility = visibility;
  return current() ? 'changed' : 'retired';
}
