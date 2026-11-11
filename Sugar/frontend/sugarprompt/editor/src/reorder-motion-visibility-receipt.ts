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
/** Exact synchronous supported-CM visibility transitions never exempt arbitrary managed mutation. */
import { requireValue } from '../../core/src/required.js';
import type { EditorView } from '@codemirror/view';
import type { ReorderMotionMask } from './reorder-motion-mask.js';
import { reorderPreview } from './reorder-preview.js';
import { currentReorderProjectionTargets } from './reorder-paint-projection.js';
import {
  reorderMotionLocalAttribute,
  reorderVisibilityValue,
} from './reorder-chip-presentation.js';
/** A recorded transition is derived output, not another roster or geometry owner. */
interface VisibilityTransition {
  readonly node: HTMLElement;
  readonly before: string | null;
  readonly after: string | null;
}
/** Capture only the actual old/new affected wrapper roster under the canonical lease. */
export function prepareReorderVisibilityReceipt(
  view: EditorView,
  next: ReorderMotionMask,
  current: () => boolean,
): ((records: readonly MutationRecord[] | null) => boolean) | null {
  if (!current()) return null;
  const targets = currentReorderProjectionTargets(view),
    old = view.state.field(reorderPreview).mask;
  if (!targets || !current()) return null;
  if (next.coverage === 'value') {
    const publication = view.state.field(reorderPreview),
      anchor = publication.paint,
      pointer = publication.pointer,
      style = view.contentDOM.getAttribute('style');
    if (!pointer || pointer.mode !== 'stage' || anchor !== pointer.anchor || !current())
      return null;
    return (records) =>
      current() &&
      (!records || records.length === 0) &&
      view.state.field(reorderPreview).pointer === pointer &&
      view.state.field(reorderPreview).paint === anchor &&
      view.contentDOM.getAttribute('style') === style &&
      currentReorderProjectionTargets(view) === targets &&
      current();
  }
  const content = view.contentDOM,
    beforeStyle = content.getAttribute('style'),
    height = view.contentHeight,
    scale = view.scaleY;
  if (!current() || !Number.isFinite(height) || height < 0 || !Number.isFinite(scale) || scale <= 0)
    return null;
  const serializer = content.ownerDocument.createElement('span').style;
  if (!current()) return null;
  serializer.cssText = beforeStyle ?? '';
  if (!current() || serializer.height) return null;
  const base = serializer.cssText;
  if (!current() || base !== (beforeStyle ?? '')) return null;
  serializer.height = `${height / scale}px`;
  if (!current()) return null;
  const locked = serializer.cssText;
  serializer.height = '';
  const restored = serializer.cssText;
  if (!current() || restored !== base) return null;
  const native = view.state.field(reorderPreview).native;
  const ids = new Set([...(old?.ids ?? []), ...next.ids]),
    missing = new Set(ids),
    transitions: VisibilityTransition[] = [],
    checks: VisibilityTransition[] = [];
  for (const target of targets) {
    if (target.kind !== 'chip' || !ids.has(target.index)) continue;
    if (!current()) return null;
    const before = target.node.getAttribute(reorderMotionLocalAttribute);
    if (!current() || before !== (native ? null : reorderVisibilityValue(old, target.index)))
      return null;
    const after = native ? null : reorderVisibilityValue(next, target.index);
    const entry = { node: target.node, before, after };
    checks.push(entry);
    missing.delete(target.index);
    if (before !== after) transitions.push(entry);
  }
  if (!current() || missing.size) return null;
  return (records) => {
    if (!current() || currentReorderProjectionTargets(view) !== targets) return false;
    if (records !== null) {
      if (records.length !== transitions.length + 2) return false;
      for (const [at, entry] of transitions.entries()) {
        const record = requireValue(records[at], 'reorder-motion-visibility-receipt');
        if (
          record.type !== 'attributes' ||
          record.attributeName !== reorderMotionLocalAttribute ||
          record.target !== entry.node ||
          record.oldValue !== entry.before
        )
          return false;
      }
      const lock = requireValue(records[transitions.length], 'reorder-motion-visibility-receipt'),
        restore = requireValue(
          records[transitions.length + 1],
          'reorder-motion-visibility-receipt',
        );
      if (
        lock.type !== 'attributes' ||
        restore.type !== 'attributes' ||
        lock.target !== content ||
        restore.target !== content ||
        lock.attributeName !== 'style' ||
        restore.attributeName !== 'style' ||
        lock.oldValue !== beforeStyle ||
        restore.oldValue !== locked
      )
        return false;
    }
    const finalStyle = content.getAttribute('style');
    if (!current() || (finalStyle ?? '') !== restored) return false;
    for (const entry of checks) {
      const value = entry.node.getAttribute(reorderMotionLocalAttribute);
      if (!current() || value !== entry.after) return false;
    }
    return current() && currentReorderProjectionTargets(view) === targets;
  };
}
