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
/** Only actual eligibility operands enter the shared pure independent-island predicate. */
import { requireValue } from '../../core/src/required.js';
export type AtomicReorderPaintRole =
  | {
      readonly kind: 'text';
      readonly text: string;
    }
  | {
      readonly kind: 'element';
      readonly display: string;
      readonly position: string;
      readonly inset: string;
      readonly transform: string;
      readonly buffer: boolean;
      readonly generatedOutside: boolean | null;
      readonly hasMedia: boolean;
      readonly vector: boolean;
      readonly tag: string;
      readonly children: readonly Node[];
    };
/** Find one complete independent inline island while retaining every outer chrome and outside-shaping refusal shared by press and Scene copies. */
export function atomicReorderPaintIsland(
  root: Node,
  read: (node: Node) => AtomicReorderPaintRole | undefined,
): Node | null {
  const descriptor = read(root);
  // The fragment writer replaces these root declarations; unknown effective formatting stays on the complete-line route.
  if (
    !descriptor ||
    descriptor.kind !== 'element' ||
    descriptor.display !== 'inline' ||
    (descriptor.position !== 'static' &&
      !(descriptor.position === 'relative' && descriptor.inset === '0px')) ||
    descriptor.transform !== 'none'
  )
    return null;
  const pending: Node[] = [root];
  let island: Node | null = null;
  while (pending.length) {
    const node = requireValue(pending.pop(), 'reorder-atomic-paint-island'),
      part = read(node);
    if (!part) return null;
    if (part.kind === 'text') {
      if (part.text.length) return null;
      continue;
    }
    if (['inline-block', 'inline-flex', 'inline-grid'].includes(part.display)) {
      if (island !== null) return null;
      island = node;
      // Nested formatting belongs to this complete copied island, not another outside shaping context.
      continue;
    }
    if (part.generatedOutside !== false || part.hasMedia || part.vector || part.tag === 'br')
      return null;
    if (!part.buffer && part.tag !== 'wbr' && part.display !== 'inline') return null;
    pending.push(...part.children);
  }
  return island;
}
