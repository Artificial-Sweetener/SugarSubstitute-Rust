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
/** Native reflected spelling and canonical CSSOM output share one coherent declaration witness. */
/** A cold CSSOM output retains optional reflected spelling; only a non-null spelling admits warm reuse. */
export interface DeclarationWitness {
  readonly text: string;
  readonly spelling: string | null;
}
/** Bracket canonical serialization with native reflection so a delegated read cannot pair an old declaration with new spelling. */
export function captureDeclarations(
  node: HTMLElement,
  style: CSSStyleDeclaration,
  current: () => boolean,
): DeclarationWitness | null {
  const before = node.getAttribute('style');
  if (!current() || node.style !== style || !current()) return null;
  const text = style.cssText;
  if (!current() || node.style !== style || !current()) return null;
  const after = node.getAttribute('style');
  if (!current() || before !== after || node.style !== style || !current()) return null;
  return { text, spelling: after };
}
/** Native style reflection synchronizes CSSOM mutations; changed or absent spelling retains the original canonical admission. */
export function matchesDeclarations(
  node: HTMLElement,
  text: string,
  spelling: string | null,
  current: () => boolean,
): boolean {
  const style = node.style;
  if (!current()) return false;
  const actual = node.getAttribute('style');
  if (!current() || node.style !== style || !current()) return false;
  if (spelling !== null && actual === spelling) return true;
  const canonical = style.cssText;
  return current() && canonical === text && node.style === style && current();
}
