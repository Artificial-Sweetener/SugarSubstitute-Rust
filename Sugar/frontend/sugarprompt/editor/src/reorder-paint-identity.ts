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
/** Weak source-free display identities isolate native CSS masks and SVG resources across simultaneous editors. */
const names = new WeakMap<object, string>();
let nextName = 0;
/** An exact display issue keeps one name until its finite owner becomes unreachable. */
export function reorderPaintIssueName(issue: object): string {
  const retained = names.get(issue);
  if (retained) return retained;
  if (nextName === Number.MAX_SAFE_INTEGER) throw new Error('reorder-paint-identity-exhausted');
  const name = String(++nextName);
  names.set(issue, name);
  return name;
}
