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
/** Actual canonical/service membership and native range visibility prove spelling projection without browser paint claims. */
import assert from 'node:assert/strict';
import test from 'node:test';
import { Transaction, type EditorState } from '@codemirror/state';
import { spellingFixture } from '../unit/spelling-provider-fixture.ts';
import {
  spellingDiagnostics,
  spellingDiagnosticAt,
  currentSpellingDiagnostic,
  SpellingDiagnosticValue,
  installSpellingDiagnostics,
  prepareNativeSpellingDiagnostics,
} from '../../../../../../frontend/sugarprompt/editor/src/spelling-diagnostics.ts';
import {
  semanticStructures,
  installSemanticStructure,
  prepareNativeStructure,
} from '../../../../../../frontend/sugarprompt/editor/src/structure.ts';
import { prepareSpellingCandidates } from '../../../../../../frontend/sugarprompt/services/src/diagnostics/spelling-preparation.ts';
import type { SpellingLookup } from '../../../../../../frontend/sugarprompt/services/src/diagnostics/spelling-contracts.ts';

test('actual populated service candidates and native findings reuse complete canonical scope output', async () => {
  const f = spellingFixture();
  try {
    await f.check();
    const actual = f.state().field(spellingDiagnostics)!;
    assert.equal(actual.status, 'ready');
    assert.ok(actual.snapshot);
    assert.equal(
      actual.snapshot.prepared.scopes,
      f.state().field(semanticStructures).native!.scopes,
    );
    assert.equal(
      actual.snapshot.prepared.groups[0]!.scope,
      actual.snapshot.prepared.scopes.scopes[0],
    );
    assert.ok(actual.snapshot.prepared.groups[0]!.candidates.length > 0);
    const value = spellingDiagnosticAt(f.state(), 1)!;
    assert.ok(value);
    assert.deepEqual([value.from, value.to], [0, 4]);
    assert.equal(value.issue, actual.snapshot.issues[0]);
    await f.check();
    assert.equal(f.state().field(spellingDiagnostics), actual);
    assert.equal(f.counts.checks, 1);
  } finally {
    f.dispose();
  }
});
test('CSV quoted Unicode and CRLF findings use storage then actual logical conversion while headers remain protected', async () => {
  const source = 'Name,Prompt\r\nentry,"helo, 🦊"\r\nother,"tail"',
    f = spellingFixture(source, false, 'wildcard-csv');
  try {
    await f.check();
    const index = f.state().doc.toString().indexOf('helo'),
      value = spellingDiagnosticAt(f.state(), index + 1)!;
    assert.ok(value);
    assert.deepEqual([value.from, value.to], [index, index + 4]);
    assert.equal(value.issue.range.from, source.indexOf('helo'));
    assert.equal(value.issue.candidate.range.from, 0);
    assert.equal(spellingDiagnosticAt(f.state(), 1), null);
  } finally {
    f.dispose();
  }
});
test('source-neutral native selection changes active-token visibility without renewing issue authority', async () => {
  const f = spellingFixture();
  try {
    await f.check();
    const value = spellingDiagnosticAt(f.state(), 1)!;
    f.view.dispatch({
      selection: { anchor: 2 },
      annotations: Transaction.addToHistory.of(false),
    });
    assert.equal(spellingDiagnosticAt(f.state(), 1), null);
    assert.equal(currentSpellingDiagnostic(f.state(), value), true);
    assert.equal(f.state().field(spellingDiagnostics)!.muted, value);
    f.view.dispatch({
      selection: { anchor: 4 },
      annotations: Transaction.addToHistory.of(false),
    });
    assert.equal(spellingDiagnosticAt(f.state(), 1), value);
    f.view.dispatch({
      selection: { anchor: 5 },
      annotations: Transaction.addToHistory.of(false),
    });
    assert.equal(spellingDiagnosticAt(f.state(), 1), value);
  } finally {
    f.dispose();
  }
});
test('foreign issue and copied native publication cannot replace genuine indexed authority', async () => {
  const f = spellingFixture();
  try {
    await f.check();
    const actual = f.state().field(spellingDiagnostics)!,
      value = spellingDiagnosticAt(f.state(), 1)!;
    const foreign = new SpellingDiagnosticValue(
      value.session,
      value.snapshot,
      value.issue,
      value.from,
      value.to,
      value.committedAtEnd,
    );
    assert.equal(currentSpellingDiagnostic(f.state(), foreign), false);
    f.view.dispatch({
      effects: installSpellingDiagnostics.of({ ...actual }),
      annotations: Transaction.addToHistory.of(false),
    });
    assert.equal(f.state().field(spellingDiagnostics), actual);
    assert.equal(
      prepareNativeSpellingDiagnostics(f.state(), f.session, {
        ...value.snapshot,
      }),
      null,
    );
  } finally {
    f.dispose();
  }
});
test('late accepted installation after shared session ignore cannot restore suppressed marks', async () => {
  const f = spellingFixture();
  try {
    await f.check();
    const old = f.state().field(spellingDiagnostics)!,
      value = spellingDiagnosticAt(f.state(), 1)!;
    await f.provider.operate(value, 'ignore');
    f.view.dispatch({
      effects: installSpellingDiagnostics.of(old),
      annotations: Transaction.addToHistory.of(false),
    });
    assert.equal(f.state().field(spellingDiagnostics), null);
    await f.check();
    assert.equal(f.state().field(spellingDiagnostics)!.status, 'ready');
    assert.equal(f.state().field(spellingDiagnostics)!.findings.size, 0);
  } finally {
    f.dispose();
  }
});

/** Observe real projected UTF16 intervals without treating paint as actionable finding membership. */
function paintedSpellingRanges(state: EditorState): readonly (readonly number[])[] {
  const ranges: number[][] = [];
  state.field(spellingDiagnostics)?.decorations.between(0, state.doc.length, (from, to) => {
    ranges.push([from, to]);
  });
  return ranges;
}

test('native linebreaks retain untouched spelling paint while immediately retiring old action authority', async () => {
  for (const insertAt of [0, 4, 10]) {
    const f = spellingFixture();
    try {
      await f.check();
      const old = spellingDiagnosticAt(f.state(), 1)!;
      assert.ok(old);
      f.view.dispatch({ changes: { from: insertAt, insert: '\n' } });
      const start = insertAt === 0 ? 1 : 0;
      assert.equal(currentSpellingDiagnostic(f.state(), old), false);
      assert.equal(spellingDiagnosticAt(f.state(), start + 1), null);
      assert.equal(f.state().field(spellingDiagnostics)?.snapshot ?? null, null);
      assert.equal(f.state().field(spellingDiagnostics)?.findings.size ?? 0, 0);
      assert.deepEqual(paintedSpellingRanges(f.state()), [[start, start + 4]]);
    } finally {
      f.dispose();
    }
  }
});

test('fresh canonical publication and held dictionary check preserve mapped paint until genuine replacement', async () => {
  const f = spellingFixture(),
    reply: { finish: ((value: SpellingLookup) => void) | null } = {
      finish: null,
    };
  let pending: Promise<void> | null = null;
  try {
    await f.check();
    const old = spellingDiagnosticAt(f.state(), 1)!;
    assert.ok(old);
    f.view.dispatch({ changes: { from: 0, insert: '\n' } });
    f.view.dispatch({
      effects: installSemanticStructure.of({
        purpose: 'native',
        prepared: prepareNativeStructure(f.state()),
      }),
      annotations: Transaction.addToHistory.of(false),
    });
    f.boundary.check = () =>
      new Promise((resolve) => {
        reply.finish = resolve;
      });
    pending = f.check();
    assert.ok(reply.finish);
    assert.equal(currentSpellingDiagnostic(f.state(), old), false);
    assert.equal(spellingDiagnosticAt(f.state(), 2), null);
    assert.deepEqual(paintedSpellingRanges(f.state()), [[1, 5]]);
    reply.finish({ status: 'ready', value: [false, true] });
    await pending;
    const fresh = spellingDiagnosticAt(f.state(), 2);
    assert.ok(fresh);
    assert.notEqual(fresh, old);
    assert.equal(currentSpellingDiagnostic(f.state(), old), false);
    assert.deepEqual([fresh.from, fresh.to], [1, 5]);
    assert.deepEqual(paintedSpellingRanges(f.state()), [[1, 5]]);
  } finally {
    reply.finish?.({ status: 'ready', value: [false, true] });
    await pending;
    f.dispose();
  }
});

test('native edits inside or extending a misspelled word immediately remove its old paint and authority', async () => {
  for (const change of [
    { from: 0, insert: 'x' },
    { from: 4, insert: 'x' },
    { from: 4, insert: "'" },
    { from: 2, to: 3, insert: 'x' },
    { from: 0, to: 10, insert: 'tail' },
  ]) {
    const f = spellingFixture();
    try {
      await f.check();
      const old = spellingDiagnosticAt(f.state(), 1)!;
      assert.ok(old);
      f.view.dispatch({ changes: change });
      assert.equal(currentSpellingDiagnostic(f.state(), old), false);
      assert.deepEqual(paintedSpellingRanges(f.state()), []);
    } finally {
      f.dispose();
    }
  }
});

test('native indentation replacement and its inverse retain actual mapped underline ranges', async () => {
  const f = spellingFixture('cat,  helo, bird');
  try {
    await f.check();
    assert.deepEqual(paintedSpellingRanges(f.state()), [[6, 10]]);
    f.view.dispatch({
      changes: { from: 4, to: 6, insert: '\n' },
      selection: { anchor: 5 },
    });
    assert.deepEqual(paintedSpellingRanges(f.state()), [[5, 9]]);
    f.view.dispatch({
      changes: { from: 4, to: 5, insert: '  ' },
      selection: { anchor: 4 },
    });
    assert.deepEqual(paintedSpellingRanges(f.state()), [[6, 10]]);
  } finally {
    f.dispose();
  }
});

test('carried active-word visibility uses mapped positions and compositionstart retires paint without a document edit', async () => {
  const f = spellingFixture();
  try {
    await f.check();
    f.view.dispatch({
      changes: { from: 0, insert: '\n' },
      selection: { anchor: 3 },
    });
    const active: string[] = [];
    f
      .state()
      .field(spellingDiagnostics)
      ?.decorations.between(0, f.state().doc.length, (_from, _to, mark) => {
        active.push(mark.spec.attributes['data-diagnostic-active']);
      });
    assert.deepEqual(paintedSpellingRanges(f.state()), [[1, 5]]);
    assert.deepEqual(active, ['true']);
    f.view.dispatch({ selection: { anchor: f.state().doc.length } });
    const visible: string[] = [];
    f
      .state()
      .field(spellingDiagnostics)
      ?.decorations.between(0, f.state().doc.length, (_from, _to, mark) => {
        visible.push(mark.spec.attributes['data-diagnostic-active']);
      });
    assert.deepEqual(visible, ['false']);
    const document = f.state().doc;
    f.provider.compositionStart();
    assert.equal(f.state().doc, document);
    assert.deepEqual(paintedSpellingRanges(f.state()), []);
  } finally {
    f.dispose();
  }
});

test('current canonical syntax removes untouched provisional paint before a new dictionary result', async () => {
  const f = spellingFixture('cat, helo, bird');
  try {
    await f.check();
    const old = spellingDiagnosticAt(f.state(), 6)!;
    assert.ok(old);
    f.view.dispatch({
      changes: [
        { from: 0, insert: '<lora:' },
        { from: 15, insert: ':1>' },
      ],
    });
    assert.equal(currentSpellingDiagnostic(f.state(), old), false);
    assert.deepEqual(paintedSpellingRanges(f.state()), [[11, 15]]);
    f.view.dispatch({
      effects: installSemanticStructure.of({
        purpose: 'native',
        prepared: prepareNativeStructure(f.state()),
      }),
      annotations: Transaction.addToHistory.of(false),
    });
    const canonical = prepareSpellingCandidates(f.state().field(semanticStructures).native!.scopes);
    assert.equal(
      canonical.groups.some((group) =>
        group.candidates.some((candidate) => candidate.text === 'helo'),
      ),
      false,
    );
    const pending = f.check();
    assert.deepEqual(paintedSpellingRanges(f.state()), []);
    await pending;
    assert.deepEqual(paintedSpellingRanges(f.state()), []);
    assert.equal(spellingDiagnosticAt(f.state(), 12), null);
  } finally {
    f.dispose();
  }
});
