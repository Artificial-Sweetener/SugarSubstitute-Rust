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
/** Stateless chip presentation belongs to the supported canonical Widget callback. */
import { ReorderPaintRetired } from './reorder-widget-block.js';
import type { MountedReorderTarget } from './reorder-mounted-target.js';
import type { ReorderPreview } from './reorder-preview-contracts.js';
import type { ReorderMotionMask } from './reorder-motion-mask.js';
import { reorderPaintIssueName } from './reorder-paint-identity.js';

/** Local visibility has no geometry or semantic authority; StateField.mask owns its exact descriptor. */
export const reorderMotionLocalAttribute = 'data-reorder-motion-local';

/** Only the exact StateField descriptor may skip its supported Widget metadata update. */
export function sameReorderVisibility(
  a: ReorderMotionMask | null,
  b: ReorderMotionMask | null,
): boolean {
  return a === b;
}

/** Native hover remains authoritative; an inactive gate makes even retained old local attributes inert. */
export function reorderVisibilityValue(mask: ReorderMotionMask | null, id: number): string | null {
  return mask?.ids.includes(id) ? reorderPaintIssueName(mask.scope) : null;
}

/** Apply only owned classes and visibility metadata, fencing every native accessor/write through the actual mounted owner. */
export function presentReorderChip(
  node: HTMLElement,
  id: number,
  preview: ReorderPreview,
  mask: ReorderMotionMask | null,
  current: () => void,
): void {
  const classes =
    id === preview.activeChip
      ? 'sp-reorder-chip sp-reorder-chip-active' +
        (preview.presentation === 'pointer' ? ' sp-reorder-held' : '')
      : 'sp-reorder-chip';
  current();
  const previous = node.className;
  current();
  if (previous !== classes) node.className = classes;
  current();
  const value = reorderVisibilityValue(mask, id),
    old = node.getAttribute(reorderMotionLocalAttribute);
  current();
  if (old !== value) {
    if (value === null) node.removeAttribute(reorderMotionLocalAttribute);
    else node.setAttribute(reorderMotionLocalAttribute, value);
    current();
  }
}
/** The complete mask-only supported callback preserves canonical resources and retires only its still-owned output on unexpected failure. */
export function updateReorderChipPresentation(
  targets: readonly MountedReorderTarget[],
  preview: ReorderPreview,
  mask: ReorderMotionMask | null,
  current: () => boolean,
  retire: () => void,
): boolean {
  const check = (): void => {
    if (!current()) throw new ReorderPaintRetired();
  };
  try {
    for (const target of targets)
      if (target.kind === 'chip')
        presentReorderChip(target.node, target.index, preview, mask, check);
    check();
    return true;
  } catch (primary) {
    if (primary instanceof ReorderPaintRetired) return false;
    try {
      retire();
    } catch (cleanup) {
      throw new AggregateError([primary, cleanup], 'Reorder visibility cleanup failed');
    }
    throw primary;
  }
}

/** Root-issued native metadata and actual canonical classes share one current presentation selector contract. */
export function reorderChipPresentationSelector(
  value: string | null,
  id: string | null,
  presentation: 'all' | 'active' | 'held' | 'unheld',
): string {
  const canonical =
    '.sp-reorder-chip:not([data-reorder-native-value])' +
    (presentation === 'all'
      ? ''
      : presentation === 'active'
        ? '.sp-reorder-chip-active'
        : presentation === 'held'
          ? '.sp-reorder-held'
          : ':not(.sp-reorder-held)');
  if (value === null) return canonical;
  if (!/^(0|[1-9][0-9]*)$/u.test(value) || id === null || !/^(0|[1-9][0-9]*)$/u.test(id))
    throw new Error('reorder-native-presentation-identity-invalid');
  const root = '.cm-editor[data-reorder-native-value="' + value + '"]';
  const gate =
    presentation === 'all'
      ? ''
      : presentation === 'active'
        ? '[data-reorder-native-active="' + id + '"]'
        : presentation === 'held'
          ? '[data-reorder-native-active="' + id + '"][data-reorder-native-held="true"]'
          : ':not([data-reorder-native-active="' + id + '"][data-reorder-native-held="true"])';
  return (
    root + gate + ' [data-reorder-native-value="' + value + '"][data-reorder-chip="' + id + '"]'
  );
}
/** Native chip and carrier membership borrow the same actual root-issued value identity without another scope index. */
export function nativeReorderPresentationMember(node: Element): boolean {
  const value = node.getAttribute('data-reorder-native-value');
  return (
    value !== null &&
    /^(0|[1-9][0-9]*)$/u.test(value) &&
    node.closest('.cm-editor')?.getAttribute('data-reorder-native-value') === value
  );
}
/** Current actual node membership adapts root presentation without retaining another active/held state. */
export function readReorderChipPresentation(
  node: Element,
): { readonly active: boolean; readonly held: boolean } | null {
  const id = node.getAttribute('data-reorder-chip'),
    value = node.getAttribute('data-reorder-native-value');
  if (id === null || !/^(0|[1-9][0-9]*)$/u.test(id)) return null;
  if (value !== null && !nativeReorderPresentationMember(node)) return null;
  if (value !== null) {
    const editor = node.closest('.cm-editor'),
      active = editor?.getAttribute('data-reorder-native-active') === id;
    return {
      active,
      held: active && editor?.getAttribute('data-reorder-native-held') === 'true',
    };
  }
  return {
    active: node.matches(reorderChipPresentationSelector(value, id, 'active')),
    held: node.matches(reorderChipPresentationSelector(value, id, 'held')),
  };
}
