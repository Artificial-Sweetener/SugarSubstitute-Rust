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
/** One finite menu owner follows genuine editable leaves across actual native metadata transactions. */
import type { EditorState, Transaction } from '@codemirror/state';
import type { ContextMenuItem } from './context-menu-contracts.js';
import type { ContextMenu, PreparedContextMenu } from './context-menu-state.js';
import { semanticStructures } from './structure.js';
import {
  prepareNativeEditContinuation,
  followNativeEditContinuation,
  type NativeEditContinuation,
} from './document-value-continuation.js';

/** A genuine trigger leaf retains the existing action provider's exact identity. */
type SourceEditLeaf = Extract<ContextMenuItem, { kind: 'source-edit' }>;

/** One pending or visible presentation owns only finite native continuation handles. */
interface MenuEditBinding {
  readonly menu: PreparedContextMenu;
  readonly handles: Map<SourceEditLeaf, NativeEditContinuation>;
}

/** Pending measurement and visible presentation share one finite leaf registry; the native continuation owner decides every transition. */
export class ContextMenuEdits {
  private pending: MenuEditBinding | null = null;
  private visible: MenuEditBinding | null = null;

  /** Capture genuine current edits off input without reconstructing their transaction or authoring effects. */
  prepare(state: EditorState, menu: PreparedContextMenu): boolean {
    this.pending = null;
    const handles = new Map<SourceEditLeaf, NativeEditContinuation>();
    const leaves = new Set([
      ...(menu.actions?.editLeaves ?? []),
      ...(menu.diagnostic?.kind === 'duplicate-segment' || menu.diagnostic?.kind === 'spelling'
        ? menu.diagnostic.editLeaves
        : []),
    ]);
    if (leaves.size) {
      const native = state.field(semanticStructures).native;
      if (!native) return false;
      const context = {
        values: native.nativeValues,
        semantics: native.documentSemantics,
        enabled: !state.readOnly,
        composing: false,
      };
      for (const leaf of leaves) {
        const prepared = prepareNativeEditContinuation(state, context, leaf.replacement);
        if (prepared.status !== 'ready') return false;
        handles.set(leaf, prepared.value);
      }
    }
    this.pending = { menu, handles };
    return true;
  }

  /** Follow every actual transaction in order, including source-neutral open, close and observer publication. */
  follow(transactions: readonly Transaction[]): void {
    for (const transaction of transactions)
      for (const binding of [this.pending, this.visible]) {
        if (!binding) continue;
        for (const [leaf, handle] of binding.handles) {
          const next = followNativeEditContinuation(handle, transaction);
          if (next.status === 'ready') binding.handles.set(leaf, next.value);
          else binding.handles.delete(leaf);
        }
      }
  }

  /** Bind the measured presentation only to its exact original action, row and text-menu identities. */
  publish(menu: ContextMenu): boolean {
    const pending = this.pending;
    if (
      !pending ||
      pending.menu.actions !== menu.actions ||
      pending.menu.items !== menu.items ||
      pending.menu.text !== menu.text
    )
      return false;
    this.visible = { menu, handles: pending.handles };
    this.pending = null;
    return true;
  }

  /** Consume one exact visible action leaf before dismissal releases the remaining finite menu lifetime. */
  take(menu: ContextMenu, leaf: SourceEditLeaf): NativeEditContinuation | null {
    const binding = this.visible;
    if (binding?.menu !== menu) return null;
    const handle = binding.handles.get(leaf) ?? null;
    binding.handles.delete(leaf);
    return handle;
  }

  /** Retire an obsolete pending measurement while preserving an independently current visible menu. */
  cancelPending(): void {
    this.pending = null;
  }

  /** Native state retirement releases current editable leaves without adopting source or caret ownership. */
  retireVisible(): void {
    this.visible = null;
  }

  /** Dismissal and teardown release both finite presentations. */
  clear(): void {
    this.pending = null;
    this.visible = null;
  }
}
