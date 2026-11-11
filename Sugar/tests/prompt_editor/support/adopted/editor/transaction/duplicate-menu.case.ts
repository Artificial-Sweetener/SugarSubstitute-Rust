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
/** Genuine value proposals and native continuation prove duplicate source, selection, authorship and history ownership. */
import assert from 'node:assert/strict';
import test from 'node:test';
import { EditorSelection, StateEffect, EditorState } from '@codemirror/state';
import { undo, redo, undoDepth } from '@codemirror/commands';
import { structureState } from './document-structure-fixture.ts';
import {
  prepareNativeStructure,
  installSemanticStructure,
  semanticStructures,
} from '../../../../../../frontend/sugarprompt/editor/src/structure.ts';
import {
  duplicateDiagnostics,
  prepareNativeDuplicates,
  installNativeDuplicates,
  currentDuplicateDiagnostic,
} from '../../../../../../frontend/sugarprompt/editor/src/duplicate-diagnostics.ts';
import {
  duplicateIgnores,
  ignoreDuplicate,
} from '../../../../../../frontend/sugarprompt/editor/src/duplicate-ignore.ts';
import {
  duplicateMenuTargetAt,
  prepareDuplicateMenu,
  currentDuplicateMenu,
} from '../../../../../../frontend/sugarprompt/editor/src/duplicate-menu.ts';
import {
  contextMenuTargetAt,
  prepareContextMenu,
} from '../../../../../../frontend/sugarprompt/editor/src/context-menu-preparation.ts';
import { ContextMenuEdits } from '../../../../../../frontend/sugarprompt/editor/src/context-menu-edits.ts';
import {
  contextMenu,
  installContextMenu,
  type ContextMenu,
} from '../../../../../../frontend/sugarprompt/editor/src/context-menu-state.ts';
import { richProjection } from '../../../../../../frontend/sugarprompt/editor/src/rich-projection.ts';
import { loraPresentation } from '../../../../../../frontend/sugarprompt/editor/src/lora-presentation.ts';
import { loraMedia } from '../../../../../../frontend/sugarprompt/editor/src/lora-media-state.ts';
import { sceneErrors } from '../../../../../../frontend/sugarprompt/editor/src/scene-errors.ts';
import {
  followNativeEditContinuation,
  nativeEditContinuationTransaction,
} from '../../../../../../frontend/sugarprompt/editor/src/document-value-continuation.ts';
import { prepareDocumentValueDuplicate } from '../../../../../../frontend/sugarprompt/editor/src/document-value-ingestion.ts';
import { nativeDocumentValueAt } from '../../../../../../frontend/sugarprompt/editor/src/document-values.ts';
import {
  authorshipExtension,
  authorship,
  installAuthorship,
} from '../../../../../../frontend/sugarprompt/editor/src/authorship.ts';
import {
  preparedAuthorship,
  generatedShells,
  literalIntents,
  type Authorship,
} from '../../../../../../frontend/sugarprompt/editor/src/authorship-ranges.ts';
import {
  readExactSource,
  exactReplacement,
} from '../../../../../../frontend/sugarprompt/editor/src/exact-source.ts';

/** Use the actual canonical owner and its state effects; fixtures supply only literal source/selection. */
function fixture(
  source: string,
  position: number,
  kind: 'prompt' | 'wildcard-csv' = 'prompt',
  selection = EditorSelection.single(0),
  provenance: Authorship | null = null,
) {
  const f = structureState(source, kind, [
    duplicateIgnores,
    duplicateDiagnostics,
    authorshipExtension,
    richProjection,
    loraPresentation,
    loraMedia,
    sceneErrors,
    contextMenu,
  ]);
  let state = f.state.update({ selection }).state;
  if (provenance) state = state.update({ effects: installAuthorship.of(provenance) }).state;
  const native = prepareNativeStructure(state);
  state = state.update({
    effects: [
      installSemanticStructure.of({ purpose: 'native', prepared: native }),
      installNativeDuplicates.of(prepareNativeDuplicates(state, native.nativeValues)),
    ],
  }).state;
  const target = contextMenuTargetAt(state, position)!;
  const menu = prepareContextMenu(state, target, 'local')!;
  assert.equal(menu.diagnostic?.kind, 'duplicate-segment');
  return { ...f, state, menu };
}

/** Follow the actual open/close transactions and compile exactly one consumed genuine command at the current tip. */
function execute(state: EditorState, prepared: ReturnType<typeof fixture>['menu'], label: string) {
  const owner = new ContextMenuEdits();
  assert.equal(owner.prepare(state, prepared), true);
  const menu: ContextMenu = {
    ...prepared,
    anchor: { left: 1, right: 1, top: 1, bottom: 1 },
  };
  assert.equal(owner.publish(menu), true);
  const opening = state.update({ effects: installContextMenu.of(menu) });
  owner.follow([opening]);
  const leaf = menu.items.find((item) => item.kind === 'source-edit' && item.label === label);
  assert.ok(leaf?.kind === 'source-edit');
  const handle = owner.take(menu, leaf);
  assert.ok(handle);
  assert.equal(owner.take(menu, leaf), null);
  owner.clear();
  const close = opening.state.update({ effects: installContextMenu.of(null) });
  const advanced = followNativeEditContinuation(handle, close);
  assert.equal(advanced.status, 'ready');
  if (advanced.status !== 'ready') throw new Error('fixture-continuation-refused');
  const accepted = nativeEditContinuationTransaction(close.state, advanced.value);
  assert.equal(accepted.status, 'ready');
  if (accepted.status !== 'ready') throw new Error('fixture-publication-refused');
  assert.equal(accepted.value.startState, close.state);
  assert.equal(accepted.value.isUserEvent('input.duplicate'), true);
  assert.equal(
    currentDuplicateMenu(
      accepted.value.state,
      menu.diagnostic as ReturnType<typeof prepareDuplicateMenu> & object,
    ),
    false,
  );
  return accepted.value.state;
}

test('remove and emphasize-first preserve actual Core-mapped directional selection and one native undo event', () => {
  for (const [label, expected, selection] of [
    ['Remove duplicate', 'cat, tail', [2, 0]],
    ['Emphasize first', '(cat:1.30), tail', [3, 1]],
  ] as const) {
    const f = fixture('cat, (cat:1.20), tail', 5, 'prompt', EditorSelection.single(2, 0));
    assert.deepEqual(
      f.menu.diagnostic!.items.map((item) => (item.kind === 'separator' ? null : item.label)),
      ['Remove duplicate', 'Emphasize first', 'Ignore duplicate'],
    );
    let state = execute(f.state, f.menu, label);
    assert.equal(readExactSource(state), expected);
    assert.deepEqual([state.selection.main.anchor, state.selection.main.head], selection);
    assert.equal(undoDepth(state), 1);
    assert.equal(
      undo({
        get state() {
          return state;
        },
        dispatch: (transaction) => {
          state = transaction.state;
        },
      }),
      true,
    );
    assert.equal(readExactSource(state), 'cat, (cat:1.20), tail');
    assert.deepEqual([state.selection.main.anchor, state.selection.main.head], [2, 0]);
    assert.equal(
      redo({
        get state() {
          return state;
        },
        dispatch: (transaction) => {
          state = transaction.state;
        },
      }),
      true,
    );
    assert.equal(readExactSource(state), expected);
  }
});

test('quoted CSV decoded recipes preserve header, neighboring values, nonBMP selection and exact CRLF', () => {
  const source = 'name,prompt\r\nrow,"😀, 😀"\r\nnext,cat';
  const f = fixture(source, 21, 'wildcard-csv', EditorSelection.single(19, 17));
  const state = execute(f.state, f.menu, 'Emphasize first');
  assert.equal(readExactSource(state), 'name,prompt\r\nrow,"(😀:1.10)"\r\nnext,cat');
  assert.deepEqual([state.selection.main.anchor, state.selection.main.head], [26, 18]);
  assert.equal(state.field(authorship).literals.size, 0);
});

test('new source, permission, ignore and foreign target/finding retire genuine commands', () => {
  const f = fixture('cat, cat, tail', 5);
  const target = duplicateMenuTargetAt(f.state, 5)!;
  const menu = prepareDuplicateMenu(f.state, target)!;
  const native = f.state.field(semanticStructures).native!;
  const context = {
    values: native.nativeValues,
    semantics: native.documentSemantics,
    enabled: true,
    composing: false,
  };
  const admitted = nativeDocumentValueAt(f.state, context, { from: 5, to: 8 });
  assert.equal(admitted.status, 'ready');
  if (admitted.status !== 'ready') return;
  assert.equal(
    prepareDocumentValueDuplicate(
      f.state,
      context,
      { ...admitted.target },
      target.finding,
      'remove',
    ),
    null,
  );
  assert.equal(
    currentDuplicateDiagnostic(
      f.state,
      Object.assign(
        Object.create(Object.getPrototypeOf(target.finding)) as typeof target.finding,
        target.finding,
      ),
    ),
    false,
  );
  assert.equal(Reflect.set(target.finding, 'diagnostic', {}), false);
  const ignored = f.state.update({
    effects: ignoreDuplicate.of(target.finding.ignoreKey),
  }).state;
  assert.equal(currentDuplicateMenu(ignored, menu), false);
  assert.equal(
    prepareDocumentValueDuplicate(ignored, context, admitted.target, target.finding, 'remove'),
    null,
  );
  const changed = f.state.update(exactReplacement(f.state, 0, 3, 'dog')).state;
  assert.equal(currentDuplicateMenu(changed, menu), false);
  const disabled = f.state.update({
    effects: f.permission.reconfigure(EditorState.readOnly.of(true)),
  }).state;
  assert.equal(currentDuplicateMenu(disabled, menu), false);
  const owner = new ContextMenuEdits();
  assert.equal(owner.prepare(f.state, f.menu), true);
  owner.clear();
  assert.equal(
    owner.publish({
      ...f.menu,
      anchor: { left: 0, right: 0, top: 0, bottom: 0 },
    }),
    false,
  );
});

test('actual metadata-only transactions retain command membership while cross-value selection refuses source edits', () => {
  const f = fixture('cat, cat, tail', 5);
  const harmless = StateEffect.define<null>();
  const state = f.state.update({ effects: harmless.of(null) }).state;
  assert.equal(
    currentDuplicateMenu(
      state,
      f.menu.diagnostic as ReturnType<typeof prepareDuplicateMenu> & object,
    ),
    true,
  );
  const csv = fixture(
    'name,prompt\nrow,"cat, cat"\nnext,dog',
    22,
    'wildcard-csv',
    EditorSelection.single(18, 31),
  );
  assert.deepEqual(
    csv.menu.diagnostic!.items.map((item) => item.kind),
    ['disabled', 'disabled', 'ignore-duplicate'],
  );
});

test('Core normalization correspondence preserves untouched generated and literal authorship through genuine native publication', () => {
  const original = preparedAuthorship(
    [{ from: 10, to: 21, contentFrom: 11, contentTo: 15, nestingDepth: 1 }],
    [{ from: 23, to: 30 }],
  );
  const f = fixture(
    'cat, cat, (tail:1.10), literal',
    5,
    'prompt',
    EditorSelection.single(0),
    original,
  );
  const state = execute(f.state, f.menu, 'Remove duplicate');
  assert.equal(readExactSource(state), 'cat, (tail:1.10), literal');
  assert.deepEqual(generatedShells(state.field(authorship)), [
    { from: 5, to: 16, contentFrom: 6, contentTo: 10, nestingDepth: 1 },
  ]);
  assert.deepEqual(literalIntents(state.field(authorship)), [{ from: 18, to: 25 }]);
});
