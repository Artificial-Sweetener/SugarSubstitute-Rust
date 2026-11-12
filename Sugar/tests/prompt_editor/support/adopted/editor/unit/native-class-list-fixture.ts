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
/** Literal token operations share the node's className; no rendering or feedback policy is modeled. */
export function nativeClassListFixture(node: {
  className: string;
}): Pick<DOMTokenList, 'contains' | 'toggle'> {
  /** Validate the same single-token input accepted by native DOMTokenList. */
  function tokens(token: string): Set<string> {
    if (token === '') throw new DOMException('Empty token', 'SyntaxError');
    if (/[\t\n\f\r ]/u.test(token))
      throw new DOMException('Whitespace token', 'InvalidCharacterError');
    return new Set(node.className.split(/[\t\n\f\r ]+/u).filter(Boolean));
  }
  return {
    /** Observe the current literal property, including replacement since acquisition. */
    contains(token: string): boolean {
      return node.className
        .split(/[\t\n\f\r ]+/u)
        .filter(Boolean)
        .includes(token);
    },
    /** Toggle one token with native optional-force and boolean-result semantics. */
    toggle(token: string, force?: boolean): boolean {
      const names = tokens(token),
        present = names.has(token),
        enabled = force === undefined ? !present : force;
      if (enabled === present) return enabled;
      if (enabled) names.add(token);
      else names.delete(token);
      node.className = [...names].join(' ');
      return enabled;
    },
  };
}
