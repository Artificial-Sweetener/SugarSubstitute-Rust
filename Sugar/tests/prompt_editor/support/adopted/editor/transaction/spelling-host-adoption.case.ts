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
/** Public dictionary adoption changes spelling authority without retiring unrelated native command context. */
import assert from 'node:assert/strict';
import test from 'node:test';
import { undoDepth, redoDepth } from '@codemirror/commands';
import { SpellcheckSession } from '../../../../../../frontend/sugarprompt/services/src/diagnostics/spelling-session.ts';
import { editorFreshness } from '../../../../../../frontend/sugarprompt/editor/src/freshness.ts';
import { spellingSession } from '../../../../../../frontend/sugarprompt/editor/src/spelling-diagnostics.ts';
import { hostFixture } from './host-document-fixture.ts';
import { spellingFixture } from '../unit/spelling-provider-fixture.ts';
import {
  contextMenuTargetAt,
  prepareContextMenu,
} from '../../../../../../frontend/sugarprompt/editor/src/context-menu-preparation.ts';
import {
  contextMenu,
  installContextMenu,
} from '../../../../../../frontend/sugarprompt/editor/src/context-menu-state.ts';
import { currentContextMenuAction } from '../../../../../../frontend/sugarprompt/editor/src/context-menu-activation-admission.ts';

/** Distinct actual session identities share only a finite, isolated dictionary transport. */
function dictionary(): SpellcheckSession {
  return new SpellcheckSession({
    identity: Symbol(),
    languageTag: 'en',
    availability: () => ({
      status: 'available',
      dictionaryAdd: false,
      sessionIgnore: false,
    }),
    checkWords: async (words) => ({
      status: 'ready',
      value: words.map(() => true),
    }),
    suggest: async () => ({ status: 'ready', value: [] }),
  });
}

test('session installation replacement and removal preserve unrelated command authority and exact native editing state', () => {
  const f = hostFixture('helo,\r\ntail'),
    first = dictionary(),
    second = dictionary();
  try {
    f.transport.dispatch({
      changes: { from: f.state().doc.length, insert: '!' },
      selection: { anchor: 3, head: 1 },
      userEvent: 'input.type',
    });
    const before = f.state(),
      freshness = before.field(editorFreshness);
    assert.equal(undoDepth(before), 1);
    for (const session of [first, second, null]) {
      assert.equal(
        f.updates.apply({
          mode: 'preserve',
          configuration: { spellcheckSession: session },
        }).status,
        'applied',
      );
      const after = f.state();
      assert.strictEqual(after.facet(spellingSession), session);
      assert.strictEqual(after.field(editorFreshness), freshness);
      assert.strictEqual(after.doc, before.doc);
      assert.strictEqual(after.selection, before.selection);
      assert.equal(f.source(), 'helo,\r\ntail!');
      assert.equal(undoDepth(after), 1);
      assert.equal(redoDepth(after), 0);
    }
    assert.equal(
      f.updates.apply({
        mode: 'preserve',
        configuration: {
          booruPostLookup: async () => ({ status: 'unavailable' }),
        },
      }).status,
      'applied',
    );
    assert.notStrictEqual(f.state().field(editorFreshness).context, freshness.context);
    assert.strictEqual(f.state().field(editorFreshness).source, freshness.source);
  } finally {
    f.destroy();
    first.dispose();
    second.dispose();
  }
});

test('dictionary retirement rejects genuine correction and ignore leaves while unrelated freshness stays identical', async () => {
  const f = spellingFixture();
  try {
    await f.check();
    const target = contextMenuTargetAt(f.state(), 1)!;
    assert.ok(target.spellingDiagnostic);
    const finding = target.spellingDiagnostic.finding,
      suggestions = await f.provider.suggest(finding);
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
    const correction = [...prepared.diagnostic.editLeaves][0]!,
      ignore = prepared.diagnostic.ignoreLeaf,
      menu = { ...prepared, anchor: { left: 0, right: 1, top: 0, bottom: 1 } };
    f.view.dispatch({ effects: installContextMenu.of(menu) });
    assert.equal(currentContextMenuAction(f.state(), menu, correction), true);
    assert.equal(currentContextMenuAction(f.state(), menu, ignore), true);
    const before = f.state();
    f.view.dispatch({ effects: f.host.reconfigure(spellingSession.of(null)) });
    assert.strictEqual(f.state().field(editorFreshness), before.field(editorFreshness));
    assert.strictEqual(f.state().doc, before.doc);
    assert.strictEqual(f.state().selection, before.selection);
    assert.equal(f.state().field(contextMenu), null);
    assert.equal(currentContextMenuAction(f.state(), menu, correction), false);
    assert.equal(currentContextMenuAction(f.state(), menu, ignore), false);
    assert.equal((await f.provider.suggest(finding)).status, 'rejected');
    await f.provider.operate(finding, 'ignore');
    assert.equal(f.counts.ignores, 0);
    assert.equal(f.counts.suggestions, 1);
    assert.equal(undoDepth(f.state()), 0);
  } finally {
    f.dispose();
  }
});
