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
/** Genuine canonical spelling leaves and native menu continuations prove source/caret/authorship/history; browser placement remains Root-owned. */
import assert from 'node:assert/strict';
import test from 'node:test';
import { EditorSelection, EditorState, Transaction } from '@codemirror/state';
import { undo, redo, undoDepth } from '@codemirror/commands';
import { spellingFixture } from '../unit/spelling-provider-fixture.ts';
import {
  spellingMenuTargetAt,
  prepareSpellingMenu,
  currentSpellingMenu,
} from '../../../../../../frontend/sugarprompt/editor/src/spelling-menu.ts';
import {
  prepareContextMenu,
  contextMenuTargetAt,
} from '../../../../../../frontend/sugarprompt/editor/src/context-menu-preparation.ts';
import { installContextMenu } from '../../../../../../frontend/sugarprompt/editor/src/context-menu-state.ts';
import { ContextMenuEdits } from '../../../../../../frontend/sugarprompt/editor/src/context-menu-edits.ts';
import {
  followNativeEditContinuation,
  nativeEditContinuationTransaction,
} from '../../../../../../frontend/sugarprompt/editor/src/document-value-continuation.ts';
import { readExactSource } from '../../../../../../frontend/sugarprompt/editor/src/exact-source.ts';
import { generatedShells } from '../../../../../../frontend/sugarprompt/editor/src/authorship-ranges.ts';
import {
  authorship,
  installAuthorship,
} from '../../../../../../frontend/sugarprompt/editor/src/authorship.ts';
import { semanticStructures } from '../../../../../../frontend/sugarprompt/editor/src/structure.ts';
import { SpellcheckSession } from '../../../../../../frontend/sugarprompt/services/src/diagnostics/spelling-session.ts';

test('actual prepared menus omit unsupported dictionary Add while retaining current correction and Ignore leaves', async () => {
  for (const dictionaryAdd of [false, true]) {
    let adds = 0;
    const session = new SpellcheckSession({
      identity: Symbol(),
      languageTag: 'en',
      availability: () => ({
        status: 'available',
        dictionaryAdd,
        sessionIgnore: false,
      }),
      checkWords: async (words) => ({
        status: 'ready',
        value: words.map((word) => word !== 'helo'),
      }),
      suggest: async () => ({ status: 'ready', value: ['hello'] }),
      ...(dictionaryAdd
        ? {
            addToDictionary: async () => {
              adds++;
              return { status: 'ready', value: true } as const;
            },
          }
        : {}),
    });
    const f = spellingFixture('helo, tail', false, 'prompt', session);
    try {
      await f.check();
      const target = spellingMenuTargetAt(f.state(), 1)!;
      assert.ok(target);
      const suggestions = await f.provider.suggest(target.finding);
      assert.equal(suggestions.status, 'ready');
      if (suggestions.status !== 'ready') assert.fail();
      const menu = prepareSpellingMenu(f.state(), target, suggestions.value)!;
      assert.ok(menu);
      assert.equal(currentSpellingMenu(f.state(), menu), true);
      assert.equal(menu.finding.snapshot.dictionaryAdd, dictionaryAdd);
      assert.deepEqual(
        menu.items.map((item) => ('label' in item ? item.label : item.kind)),
        dictionaryAdd
          ? ['hello', 'Ignore spelling', 'Add to dictionary']
          : ['hello', 'Ignore spelling'],
      );
      assert.equal(menu.addLeaf === null, !dictionaryAdd);
      if (menu.addLeaf) assert.equal(menu.items.includes(menu.addLeaf), true);
      assert.equal(menu.items.includes(menu.ignoreLeaf), true);
      assert.equal(menu.ignoreLeaf.finding, target.finding);
      assert.equal(menu.editLeaves.size, 1);
      const correction = [...menu.editLeaves][0]!;
      assert.equal(correction.label, 'hello');
      assert.equal(menu.items.includes(correction), true);
      assert.equal(readExactSource(f.state()), 'helo, tail');
      assert.equal(undoDepth(f.state()), 0);
      assert.equal(adds, 0);
      await f.provider.operate(menu.finding, 'ignore');
      await f.check();
      assert.equal(spellingMenuTargetAt(f.state(), 1), null);
      assert.equal(readExactSource(f.state()), 'helo, tail');
      assert.equal(undoDepth(f.state()), 0);
      assert.equal(adds, 0);
    } finally {
      f.dispose();
      session.dispose();
    }
  }
});

test('literal suggestion correction normalizes only in rich mode and retains one native undo/redo event', async () => {
  for (const rich of [false, true]) {
    const f = spellingFixture('helo, tail', rich);
    try {
      await f.check();
      f.boundary.suggest = async () => ({ status: 'ready', value: ['(cat)'] });
      f.view.dispatch({
        selection: EditorSelection.create([EditorSelection.range(9, 6)]),
        annotations: Transaction.addToHistory.of(false),
      });
      const target = spellingMenuTargetAt(f.state(), 1)!;
      assert.ok(target);
      const pending = prepareSpellingMenu(f.state(), target)!;
      const loading = pending.items[0]!;
      assert.equal(loading.kind, 'disabled');
      if (loading.kind !== 'disabled') assert.fail();
      assert.equal(loading.label, 'Loading spelling suggestions');
      const suggestions = await f.provider.suggest(target.finding);
      assert.equal(suggestions.status, 'ready');
      if (suggestions.status !== 'ready') assert.fail();
      const menu = prepareSpellingMenu(f.state(), target, suggestions.value)!;
      const leaf = [...menu.editLeaves][0]!;
      assert.equal(leaf.label, '(cat)');
      assert.equal(
        leaf.replacement.transactions.at(-1)!.annotation(Transaction.userEvent),
        'input.spelling',
      );
      for (const transaction of leaf.replacement.transactions) f.view.dispatch(transaction);
      assert.equal(readExactSource(f.state()), rich ? '(cat:1.10), tail' : '(cat), tail');
      assert.equal(f.state().selection.main.anchor, f.state().doc.toString().indexOf(', tail'));
      assert.equal(f.state().selection.main.head, f.state().selection.main.anchor);
      assert.equal(generatedShells(f.state().field(authorship)).length, rich ? 1 : 0);
      assert.equal(undoDepth(f.state()), 1);
      assert.equal(undo(f.view), true);
      assert.equal(readExactSource(f.state()), 'helo, tail');
      assert.deepEqual([f.state().selection.main.anchor, f.state().selection.main.head], [9, 6]);
      assert.equal(redo(f.view), true);
      assert.equal(readExactSource(f.state()), rich ? '(cat:1.10), tail' : '(cat), tail');
    } finally {
      f.dispose();
    }
  }
});
test('quoted CSV suggestions preserve commas quotes nonBMP exact endings and the independent neighboring value', async () => {
  for (const rich of [false, true]) {
    const source = 'Name,Prompt\r\nentry,"helo, tail"\r\nother,"keep"',
      f = spellingFixture(source, rich, 'wildcard-csv');
    try {
      await f.check();
      f.boundary.suggest = async () => ({
        status: 'ready',
        value: ['🦊, "literal"'],
      });
      const index = f.state().doc.toString().indexOf('helo'),
        target = spellingMenuTargetAt(f.state(), index + 1)!;
      assert.ok(target);
      const suggestions = await f.provider.suggest(target.finding);
      assert.equal(suggestions.status, 'ready');
      if (suggestions.status !== 'ready') assert.fail();
      const menu = prepareSpellingMenu(f.state(), target, suggestions.value)!;
      assert.equal(menu.editLeaves.size, 1);
      for (const tx of [...menu.editLeaves][0]!.replacement.transactions) f.view.dispatch(tx);
      assert.equal(
        readExactSource(f.state()),
        'Name,Prompt\r\nentry,"🦊, ""literal"", tail"\r\nother,"keep"',
      );
      assert.equal(f.state().selection.main.head, f.state().doc.toString().indexOf(', tail'));
      assert.equal(undo(f.view), true);
      assert.equal(readExactSource(f.state()), source);
      assert.equal(redo(f.view), true);
      assert.equal(
        readExactSource(f.state()),
        'Name,Prompt\r\nentry,"🦊, ""literal"", tail"\r\nother,"keep"',
      );
    } finally {
      f.dispose();
    }
  }
});
test('measured menu installation and close advance the same genuine spelling output and exact authored effect identity', async () => {
  const f = spellingFixture('helo, tail', true),
    registry = new ContextMenuEdits();
  try {
    await f.check();
    f.boundary.suggest = async () => ({ status: 'ready', value: ['(cat)'] });
    const target = contextMenuTargetAt(f.state(), 1)!;
    const suggestions = await f.provider.suggest(target.spellingDiagnostic!.finding);
    assert.equal(suggestions.status, 'ready');
    if (suggestions.status !== 'ready') assert.fail();
    const prepared = prepareContextMenu(
      f.state(),
      target,
      'local',
      { readable: false, writable: false },
      suggestions.value,
    )!;
    assert.equal(prepared.diagnostic?.kind, 'spelling');
    if (prepared.diagnostic?.kind !== 'spelling') assert.fail();
    const leaf = [...prepared.diagnostic.editLeaves][0]!;
    const authored = leaf.replacement.transactions
      .at(-1)!
      .effects.find((effect) => effect.is(installAuthorship));
    assert.ok(authored);
    assert.equal(registry.prepare(f.state(), prepared), true);
    const menu = {
      ...prepared,
      anchor: { left: 1, right: 1, top: 1, bottom: 1 },
    };
    assert.equal(registry.publish(menu), true);
    const open = f.state().update({
      effects: installContextMenu.of(menu),
      annotations: Transaction.addToHistory.of(false),
    });
    registry.follow([open]);
    f.view.dispatch(open);
    assert.equal(registry.take(menu, { ...leaf }), null);
    const handle = registry.take(menu, leaf);
    assert.ok(handle);
    const close = f.state().update({
      effects: installContextMenu.of(null),
      annotations: Transaction.addToHistory.of(false),
    });
    const advanced = followNativeEditContinuation(handle, close);
    assert.equal(advanced.status, 'ready');
    if (advanced.status !== 'ready') assert.fail();
    f.view.dispatch(close);
    const accepted = nativeEditContinuationTransaction(f.state(), advanced.value);
    assert.equal(accepted.status, 'ready');
    if (accepted.status !== 'ready') assert.fail();
    assert.equal(accepted.value.startState, f.state());
    assert.ok(accepted.value.effects.includes(authored));
    f.view.dispatch(accepted.value);
    assert.equal(readExactSource(f.state()), '(cat:1.10), tail');
    assert.equal(f.state().field(authorship), authored.value);
    assert.equal(undoDepth(f.state()), 1);
  } finally {
    registry.clear();
    f.dispose();
  }
});
test('same-word correction retains populated canonical output and creates no source undo event', async () => {
  const f = spellingFixture();
  try {
    await f.check();
    const native = f.state().field(semanticStructures).native!;
    f.boundary.suggest = async () => ({ status: 'ready', value: ['helo'] });
    const target = spellingMenuTargetAt(f.state(), 1)!;
    const suggestions = await f.provider.suggest(target.finding);
    assert.equal(suggestions.status, 'ready');
    if (suggestions.status !== 'ready') assert.fail();
    const menu = prepareSpellingMenu(f.state(), target, suggestions.value)!;
    const edit = [...menu.editLeaves][0]!.replacement;
    assert.equal(edit.postValues.scopes, native.scopes);
    assert.equal(edit.transactions.at(-1)!.changes.empty, true);
    for (const tx of edit.transactions) f.view.dispatch(tx);
    assert.equal(readExactSource(f.state()), 'helo, tail');
    assert.equal(f.state().field(semanticStructures).native!.scopes, native.scopes);
    assert.equal(undoDepth(f.state()), 0);
  } finally {
    f.dispose();
  }
});
test('copied suggestion target and menu objects cannot acquire genuine spelling action membership', async () => {
  const f = spellingFixture();
  try {
    await f.check();
    const target = spellingMenuTargetAt(f.state(), 1)!;
    const suggestions = await f.provider.suggest(target.finding);
    assert.equal(suggestions.status, 'ready');
    if (suggestions.status !== 'ready') assert.fail();
    assert.equal(prepareSpellingMenu(f.state(), { ...target }, suggestions.value), null);
    assert.equal(prepareSpellingMenu(f.state(), target, { ...suggestions.value }), null);
    const menu = prepareSpellingMenu(f.state(), target, suggestions.value)!;
    assert.equal(currentSpellingMenu(f.state(), menu), true);
    assert.equal(currentSpellingMenu(f.state(), { ...menu }), false);
    f.view.dispatch({ changes: { from: f.state().doc.length, insert: '!' } });
    assert.equal(currentSpellingMenu(f.state(), menu), false);
    assert.equal(prepareSpellingMenu(f.state(), target, suggestions.value), null);
  } finally {
    f.dispose();
  }
});
test('typed unavailable suggestions disclose absence and read-only menus retain no corrective source leaf', async () => {
  const f = spellingFixture();
  try {
    await f.check();
    const target = spellingMenuTargetAt(f.state(), 1)!;
    f.boundary.suggest = async () => ({
      status: 'unavailable',
      code: 'dictionary-unavailable',
    });
    const unavailable = await f.provider.suggest(target.finding);
    assert.equal(unavailable.status, 'unavailable');
    const empty = prepareSpellingMenu(f.state(), target, null, 'unavailable')!;
    const absence = empty.items[0]!;
    assert.equal(absence.kind, 'disabled');
    if (absence.kind !== 'disabled') assert.fail();
    assert.equal(absence.label, 'Spelling unavailable');
    assert.equal(empty.editLeaves.size, 0);
    assert.equal(empty.ignoreLeaf.label, 'Ignore spelling');
    assert.equal(empty.addLeaf!.label, 'Add to dictionary');
    f.view.dispatch({
      effects: f.initial.permission.reconfigure(EditorState.readOnly.of(true)),
    });
    await f.check();
    f.boundary.suggest = async () => ({ status: 'ready', value: ['hello'] });
    const current = spellingMenuTargetAt(f.state(), 1)!;
    const suggestions = await f.provider.suggest(current.finding);
    assert.equal(suggestions.status, 'ready');
    if (suggestions.status !== 'ready') assert.fail();
    const readonly = prepareSpellingMenu(f.state(), current, suggestions.value)!;
    assert.equal(readonly.editLeaves.size, 0);
    assert.equal(readonly.items[0]!.kind, 'disabled');
    assert.equal(readExactSource(f.state()), 'helo, tail');
    assert.equal(undoDepth(f.state()), 0);
  } finally {
    f.dispose();
  }
});
