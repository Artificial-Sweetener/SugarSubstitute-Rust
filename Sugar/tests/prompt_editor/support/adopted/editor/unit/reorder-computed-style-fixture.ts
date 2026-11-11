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
/** Literal resolved declarations transport the actual paint fixture's native style/box boundary. */
import type { ReorderPaintStyleFixture } from './reorder-paint-style-fixture.ts';

/** Only native declaration and independent rectangle operands enter the resolved-style transport. */
interface ComputedStyleNode {
  readonly style: ReorderPaintStyleFixture;
  readonly box: { readonly width: number; readonly height: number };
}
/** Resolve the existing finite browser DTO; production owners retain all admission decisions. */
export function motionComputedStyleFixture(node: ComputedStyleNode, pseudo?: string) {
  const properties: Record<string, string> = {
    font: '14px serif',
    content: pseudo === undefined ? 'normal' : 'none',
    filter: 'none',
    'backdrop-filter': 'none',
    'background-image': 'none',
    'background-blend-mode': 'normal',
    'box-shadow': 'none',
    'mix-blend-mode': 'normal',
    isolation: 'auto',
    'clip-path': 'none',
    'outline-style': 'none',
    'text-decoration-line': 'none',
    'text-shadow': 'none',
    backgroundImage: 'none',
    backgroundColor: 'rgba(0, 0, 0, 0)',
    visibility: 'visible',
    opacity:
      typeof node.style.opacity === 'string' && node.style.opacity !== ''
        ? node.style.opacity
        : '1',
    display: 'inline',
    width: `${node.box.width}px`,
    height: `${node.box.height}px`,
    'box-sizing': 'border-box',
    'overflow-x': String(node.style.overflowX || node.style.overflow || 'visible'),
    'overflow-y': String(node.style.overflowY || node.style.overflow || 'visible'),
    'overflow-clip-margin': String(node.style.overflowClipMargin || '0px'),
    ...Object.fromEntries(
      ['top', 'right', 'bottom', 'left'].flatMap((side) => [
        [`border-${side}-width`, '0px'],
        [`padding-${side}`, '0px'],
      ]),
    ),
  };
  const names = Object.keys(properties);
  return {
    ...properties,
    outlineColor: 'rgb(1, 2, 3)',
    opacity: properties.opacity ?? '1',
    boxSizing: 'border-box',
    length: names.length,
    item: (at: number) => names[at]!,
    getPropertyValue: (name: string) => properties[name] ?? '',
  };
}
