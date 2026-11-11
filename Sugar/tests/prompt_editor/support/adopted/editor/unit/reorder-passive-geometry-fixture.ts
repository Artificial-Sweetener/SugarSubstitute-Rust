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
/** Literal passive external scene geometry transports native calibration phases without certifying browser wrapping. */
export interface PassiveNode {
  readonly className?: string;
  readonly style?: object;
  readonly dataset?: Readonly<Record<string, string>>;
  readonly childNodes?: readonly unknown[];
  readonly parentNode?: PassiveNode | null;
}

/** Controlled external DOM positions are independent of the currently proposed live chip order. */
export function passiveGeometry(node: PassiveNode): DOMRect[] | null {
  if (
    node.className?.includes('sp-reorder-base-measurement') &&
    node.style &&
    Reflect.get(node.style, 'height') === '256px'
  ) {
    const width = Number.parseFloat(String(Reflect.get(node.style, 'width')));
    return [{ left: 0, right: width, top: 0, bottom: 256, width, height: 256 }] as DOMRect[];
  }
  let parent = node.parentNode;
  while (parent && !parent.className?.includes('sp-reorder-base-measurement'))
    parent = parent.parentNode;
  if (!parent?.style || node.dataset?.reorderChip === undefined) return null;
  const siblings = node.parentNode!.childNodes!.filter(
      (child) => (child as PassiveNode).dataset?.reorderChip !== undefined,
    ),
    at = siblings.indexOf(node),
    left = Number.parseFloat(String(Reflect.get(parent.style, 'left'))) + at * 30,
    top = Number.parseFloat(String(Reflect.get(parent.style, 'top')));
  return [{ left, right: left + 20, top, bottom: top + 20, width: 20, height: 20 }] as DOMRect[];
}
/** Literal independent chip fragments are supplied at the actual connected external collision boundary. */
export function collisionGeometry(
  fixture: { setPassiveGeometry(read: typeof passiveGeometry): void },
  read: (id: number) => DOMRect[],
): void {
  fixture.setPassiveGeometry((node) => {
    if (node.className?.includes('sp-reorder-base-measurement')) return passiveGeometry(node);
    let root = node.parentNode;
    while (root && !root.className?.includes('sp-reorder-base-measurement')) root = root.parentNode;
    return root && node.dataset?.reorderChip !== undefined
      ? read(Number(node.dataset.reorderChip))
      : null;
  });
}
