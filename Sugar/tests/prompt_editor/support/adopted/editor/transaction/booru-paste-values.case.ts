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
/** Imported tags respect value boundaries, CSV quoting, neighbouring authorship and one undo step. */
import assert from 'node:assert/strict';
import test from 'node:test';
import { undo, redo, undoDepth } from '@codemirror/commands';
import { Transaction } from '@codemirror/state';
import { authorship } from '../../../../../../frontend/sugarprompt/editor/src/authorship.ts';
import { enteredTextInsertion } from '../../../../../../frontend/sugarprompt/editor/src/ingestion.ts';
import { dispatchNativeIngestion } from '../../../../../../frontend/sugarprompt/editor/src/native-ingestion.ts';
import {
  generatedShells,
  literalIntents,
} from '../../../../../../frontend/sugarprompt/editor/src/authorship-ranges.ts';
import {
  lineEndings,
  readExactSource,
} from '../../../../../../frontend/sugarprompt/editor/src/exact-source.ts';
import { booruFixture, booruReply, lookupStarted } from './booru-paste-import-fixture.ts';
const url = 'https://danbooru.donmai.us/posts/123';

test('raw and rich quoted CSV import conserve neighboring values exact endings and original reverse selection', async () => {
  for (const rich of [false, true]) {
    const before = 'h\r\n"\ud83e\udd8a ",tail\r\nnext,last';
    const f = booruFixture(before, rich, 'wildcard-csv');
    f.view.dispatch({ selection: { anchor: 5, head: 3 } });
    const pending = f.owner.start(url);
    assert.ok(pending);
    // Nothing is pasted while the post is looked up.
    assert.equal(readExactSource(f.state()), before);
    await lookupStarted();
    f.resolve(booruReply('cat,"dog"_\ud83e\udd8a'));
    await pending;
    assert.equal(readExactSource(f.state()), 'h\r\n"cat,""dog"" \ud83e\udd8a ",tail\r\nnext,last');
    assert.equal(undoDepth(f.state()), 1);
    assert.equal(undo(f.view), true);
    assert.equal(readExactSource(f.state()), before);
    assert.equal(f.state().selection.main.anchor, 5);
    assert.equal(f.state().selection.main.head, 3);
    assert.equal(redo(f.view), true);
    assert.equal(readExactSource(f.state()), 'h\r\n"cat,""dog"" \ud83e\udd8a ",tail\r\nnext,last');
    f.destroy();
  }
});

test('header and cross-value selections stay with ordinary paste and look nothing up', () => {
  for (const [anchor, head] of [
    [2, 0],
    [6, 3],
  ] as const) {
    const f = booruFixture('h\ncat,dog', false, 'wildcard-csv');
    f.view.dispatch({ selection: { anchor, head } });
    assert.equal(f.owner.start(url), null);
    assert.equal(f.signal(), null);
    assert.equal(readExactSource(f.state()), 'h\ncat,dog');
    f.destroy();
  }
});

test('imported escaped literal source preserves genuinely generated neighboring authorship through native undo redo', async () => {
  const f = booruFixture('', true);
  const base = enteredTextInsertion(f.state(), 0, 0, '(base), ');
  assert.equal(dispatchNativeIngestion(f.view, base), true);
  const before = '(base:1.10), ';
  assert.equal(readExactSource(f.state()), before);
  const generated = generatedShells(f.state().field(authorship));
  assert.equal(generated.length, 1);
  const pending = f.owner.start(url);
  assert.ok(pending);
  await lookupStarted();
  f.resolve(booruReply('saber_(fate)'));
  await pending;
  assert.equal(readExactSource(f.state()), before + 'saber \\(fate\\)');
  assert.deepEqual(generatedShells(f.state().field(authorship)), generated);
  assert.deepEqual(literalIntents(f.state().field(authorship)), []);
  assert.equal(undoDepth(f.state()), 2);
  assert.equal(undo(f.view), true);
  assert.equal(readExactSource(f.state()), before);
  assert.deepEqual(generatedShells(f.state().field(authorship)), generated);
  assert.equal(redo(f.view), true);
  assert.equal(readExactSource(f.state()), before + 'saber \\(fate\\)');
  assert.deepEqual(generatedShells(f.state().field(authorship)), generated);
  f.destroy();
});

test('two genuine neutral post-paste links preserve exact native Text and one import undo event in raw and rich modes', async () => {
  for (const rich of [false, true]) {
    const f = booruFixture(' suffix', rich);
    try {
      const pending = f.owner.start(url);
      assert.ok(pending);
      await lookupStarted();
      const document = f.state().doc,
        endings = f.state().field(lineEndings);
      for (let link = 0; link < 2; link++) {
        f.view.dispatch(f.state().update({ annotations: Transaction.addToHistory.of(false) }));
        assert.equal(f.state().doc, document);
        assert.equal(f.state().field(lineEndings), endings);
      }
      f.resolve(booruReply());
      await pending;
      assert.equal(readExactSource(f.state()), 'cat, blue eyes suffix');
      assert.equal(undoDepth(f.state()), 1);
      assert.equal(undo(f.view), true);
      assert.equal(readExactSource(f.state()), ' suffix');
      assert.equal(redo(f.view), true);
      assert.equal(readExactSource(f.state()), 'cat, blue eyes suffix');
    } finally {
      f.destroy();
    }
  }
});
