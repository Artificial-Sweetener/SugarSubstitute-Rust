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
/** Native below-text Stage admission owns the complete actual background/generated-paint contract. */
import type { EditorView } from '@codemirror/view';

/** Below-text paint is admitted only where actual native content/line backgrounds and generated surfaces cannot obscure it. */
export function admitsReorderMotionPaintContext(
  view: EditorView,
  attachment: HTMLElement,
): boolean {
  const window = view.dom.ownerDocument.defaultView,
    lines = view.contentDOM.querySelectorAll<HTMLElement>('.cm-line');
  if (!window || !lines.length || lines.length > 16) return false;
  for (const ancestor of [view.scrollDOM, attachment]) {
    const shared = window.getComputedStyle(ancestor);
    // This genuine ancestor also encloses the copied scroller; nontrivial paint must not be applied twice.
    for (const [name, value] of [
      ['opacity', '1'],
      ['filter', 'none'],
      ['backdrop-filter', 'none'],
      ['background-image', 'none'],
      ['background-blend-mode', 'normal'],
      ['box-shadow', 'none'],
      ['mix-blend-mode', 'normal'],
      ['isolation', 'auto'],
      ['clip-path', 'none'],
      ['outline-style', 'none'],
      ['visibility', 'visible'],
      ['text-decoration-line', 'none'],
      ['text-shadow', 'none'],
    ] as const)
      if (shared.getPropertyValue(name) !== value) return false;
    if (!/^rgba\(\s*\d+\s*,\s*\d+\s*,\s*\d+\s*,\s*0\s*\)$/u.test(shared.backgroundColor))
      return false;
  }
  // Scene bands moved to the existing below-content layer; they still have no external Stage replacement painter.
  if (
    Array.from(lines).some((line) =>
      line.getAttribute('class')?.split(/\s+/u).includes('sp-scene-fill'),
    )
  )
    return false;
  for (const node of [attachment, view.contentDOM, ...lines]) {
    const style = window.getComputedStyle(node);
    if (
      style.backgroundImage !== 'none' ||
      !/^rgba\(\s*\d+\s*,\s*\d+\s*,\s*\d+\s*,\s*0\s*\)$/u.test(style.backgroundColor)
    )
      return false;
    for (const pseudo of ['::before', '::after']) {
      const generated = window.getComputedStyle(node, pseudo);
      if (
        generated.content !== 'none' &&
        generated.content !== 'normal' &&
        generated.display !== 'none' &&
        generated.opacity !== '0'
      )
        return false;
    }
  }
  return true;
}
