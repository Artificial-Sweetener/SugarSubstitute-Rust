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
/** Standard text rows prepare native history and clipboard capabilities without reading system clipboard contents. */
import { type EditorSelection, type EditorState } from '@codemirror/state';
import { redoDepth, undoDepth } from '@codemirror/commands';
import type { EditorView } from '@codemirror/view';
import { editorFreshness, type EditorFreshness } from './freshness.js';
import { nativeClipboard, runEditCommand, type ClipboardAction } from './native-clipboard.js';

/** Every native editing action runs through the prompt's one edit-command owner. */
export type TextMenuAction = ClipboardAction | 'undo' | 'redo' | 'select-all';

/** Each enabled leaf belongs to exactly one prepared native text-menu target. */
export type TextMenuItem =
  | {
      readonly kind: 'text';
      readonly action: TextMenuAction;
      readonly label: string;
      readonly shortcut: string;
    }
  | {
      readonly kind: 'disabled';
      readonly label: string;
      readonly title: string;
    }
  | { readonly kind: 'separator' };

/** Source and directional native selection capture command authority; history is consumed through supported commands. */
export interface PreparedTextMenu {
  readonly freshness: EditorFreshness;
  readonly selection: EditorSelection;
  readonly items: readonly TextMenuItem[];
  readonly leaves: ReadonlySet<Extract<TextMenuItem, { kind: 'text' }>>;
}

/** Preparation consumes static native availability and state-owned history; permission prompts occur only after activation. */
export function prepareTextMenu(
  state: EditorState,
  clipboard: { readonly readable: boolean; readonly writable: boolean },
): PreparedTextMenu {
  const items: TextMenuItem[] = [];
  const selected = !state.selection.main.empty;
  const editable = !state.readOnly;
  /** Keep disabled explanations literal and avoid authorizing a disabled command leaf. */
  function row(
    action: TextMenuAction,
    label: string,
    shortcut: string,
    enabled: boolean,
    reason: string,
  ): void {
    items.push(
      enabled
        ? { kind: 'text', action, label, shortcut }
        : { kind: 'disabled', label, title: reason },
    );
  }
  // Undo and Redo are always listed, as in every text field's edit menu.
  row(
    'undo',
    'Undo',
    'Control+Z',
    editable && undoDepth(state) > 0,
    !editable ? 'This prompt is read-only.' : 'Nothing to undo.',
  );
  row(
    'redo',
    'Redo',
    'Control+Y',
    editable && redoDepth(state) > 0,
    !editable ? 'This prompt is read-only.' : 'Nothing to redo.',
  );
  items.push({ kind: 'separator' });
  row(
    'cut',
    'Cut',
    'Control+X',
    editable && selected && clipboard.writable,
    !editable
      ? 'This prompt is read-only.'
      : !selected
        ? 'Select source text first.'
        : 'Clipboard writing is unavailable.',
  );
  row(
    'copy',
    'Copy',
    'Control+C',
    selected && clipboard.writable,
    !selected ? 'Select source text first.' : 'Clipboard writing is unavailable.',
  );
  row(
    'paste',
    'Paste',
    'Control+V',
    editable && clipboard.readable,
    !editable ? 'This prompt is read-only.' : 'Clipboard reading is unavailable.',
  );
  row('select-all', 'Select all', 'Control+A', !!state.doc.length, 'This prompt is empty.');
  return {
    freshness: state.field(editorFreshness),
    selection: state.selection,
    items: Object.freeze(items),
    leaves: new Set(
      items.filter((item): item is Extract<TextMenuItem, { kind: 'text' }> => item.kind === 'text'),
    ),
  };
}

/** A changed source/context or directional selection retires all captured standard actions. */
export function currentTextMenu(state: EditorState, menu: PreparedTextMenu): boolean {
  return menu.freshness === state.field(editorFreshness) && menu.selection.eq(state.selection);
}

/** Deliberate activation delegates to supported native commands and the shared finite clipboard owner. */
export async function executeTextMenu(
  view: EditorView,
  menu: PreparedTextMenu,
  item: Extract<TextMenuItem, { kind: 'text' }>,
): Promise<void> {
  if (
    !menu.leaves.has(item) ||
    !currentTextMenu(view.state, menu) ||
    view.composing ||
    view.compositionStarted
  )
    return;
  if (item.action === 'select-all' || item.action === 'undo' || item.action === 'redo')
    runEditCommand(view, item.action);
  else await view.plugin(nativeClipboard)?.fromInteraction(item.action);
}
