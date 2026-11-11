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
/** Measured refresh settles current input and publishes actual prepared output through finite jobs. */
import assert from 'node:assert/strict';
import test from 'node:test';
import { semanticStructures } from '../../../../../../frontend/sugarprompt/editor/src/structure.ts';
import {
  exactReplacement,
  lineEndings,
  readExactSource,
} from '../../../../../../frontend/sugarprompt/editor/src/exact-source.ts';
import { undoDepth, redoDepth } from '@codemirror/commands';
import { documentDiagnostics } from '../../../../../../frontend/sugarprompt/editor/src/document-diagnostics.ts';
import { nativeConditioning } from '../../../../../../frontend/sugarprompt/editor/src/conditioning-state.ts';
import { richProjection } from '../../../../../../frontend/sugarprompt/editor/src/rich-projection.ts';
import {
  editorFreshness,
  invalidateCommandContext,
} from '../../../../../../frontend/sugarprompt/editor/src/freshness.ts';
import { refreshFixture } from './semantic-refresh-fixture.ts';

test('boundary keys and read/write phases never parse; one after-measure task publishes actual prompt output', () => {
  const fixture = refreshFixture();
  try {
    // Only these two key fields are observed at the external keyboard-event boundary.
    const key = { key: 'ArrowLeft', ctrlKey: false };
    fixture.owner.boundaryKey(key);
    fixture.owner.pointerStart();
    assert.equal(fixture.jobs.length, 1);
    fixture.jobs[0]!.read();
    assert.equal(fixture.state().field(semanticStructures).native, null);
    fixture.jobs[0]!.write();
    assert.equal(fixture.state().field(semanticStructures).native, null);
    assert.equal(fixture.backgroundTasks(), 1);
    fixture.task();
    const prepared = fixture.state().field(semanticStructures).native!;
    assert.equal(prepared.scopes.scopes[0]!.document.emphasisSpans.length, 1);
    assert.equal(fixture.publications(), 1);
    fixture.owner.pointerStart();
    assert.equal(fixture.jobs.length, 1);
  } finally {
    fixture.destroy();
  }
});

test('source/context changes after measurement reject publication and queue current measured work', () => {
  for (const change of [
    'source',
    'context',
    'permission',
    'profile',
    'kind-return',
    'input',
  ] as const) {
    const fixture = refreshFixture();
    try {
      fixture.jobs[0]!.read();
      fixture.jobs[0]!.write();
      if (
        change === 'permission' ||
        change === 'profile' ||
        change === 'kind-return' ||
        change === 'input'
      )
        fixture.reconfigure(change);
      else
        fixture.apply(
          fixture
            .state()
            .update(
              change === 'source'
                ? exactReplacement(fixture.state(), 0, 0, 'a')
                : { effects: invalidateCommandContext.of(null) },
            ),
        );
      fixture.task();
      assert.equal(fixture.publications(), 0);
      assert.equal(fixture.jobs.length, 2);
      fixture.jobs[1]!.read();
      fixture.jobs[1]!.write();
      fixture.task();
      assert.equal(fixture.publications(), 1);
    } finally {
      fixture.destroy();
    }
  }
});

test('composition, held input and disposal retire pending measured work', () => {
  for (const retirement of ['composition', 'held', 'dispose'] as const) {
    const fixture = refreshFixture();
    try {
      fixture.jobs[0]!.read();
      fixture.jobs[0]!.write();
      if (retirement === 'composition') fixture.owner.compositionStart();
      if (retirement === 'held') fixture.hold(true);
      if (retirement === 'dispose') fixture.owner.destroy();
      fixture.task();
      assert.equal(fixture.publications(), 0);
      assert.equal(fixture.state().field(semanticStructures).native, null);
    } finally {
      fixture.destroy();
    }
  }
});

test('structured refresh publishes populated decoded CSV projection while preserving exact source selection and history', () => {
  const fixture = refreshFixture('wildcard-csv');
  try {
    const before = fixture.state(),
      history = [undoDepth(before), redoDepth(before)];
    fixture.jobs[0]!.read();
    fixture.jobs[0]!.write();
    assert.equal(fixture.state().field(semanticStructures).native, null);
    fixture.task();
    const state = fixture.state(),
      prepared = state.field(semanticStructures).native!,
      projection = state.field(richProjection),
      diagnostics = state.field(documentDiagnostics);
    assert.equal(fixture.publications(), 1);
    assert.equal(prepared.kind, 'wildcard-csv');
    assert.equal(prepared.scopes.scopes.length, 1);
    assert.equal(prepared.scopes.scopes[0]!.mapping.text === '{one}', true);
    assert.equal(prepared.nativeValues.entries[0]!.scope, null);
    assert.equal(prepared.nativeValues.entries[1]!.scope, prepared.scopes.scopes[0]);
    assert.deepEqual(
      prepared.nativeValues.entries[1]!.semanticMapping!.decodedToLogical,
      [3, 4, 5, 6, 7, 8],
    );
    assert.equal(projection.documentKind, 'wildcard-csv');
    assert.equal(projection.sourceIdentity, prepared.sourceIdentity);
    assert.equal(projection.tokens.size, 1);
    const token = projection.tokens.iter();
    assert.deepEqual([token.from, token.to], [3, 8]);
    assert.ok(projection.all.size > 0);
    for (const range = projection.all.iter(); range.value; range.next()) {
      assert.ok(range.from >= 3);
      assert.ok(range.to <= 8);
    }
    assert.equal(diagnostics.kind, 'wildcard-csv');
    assert.equal(diagnostics.source, prepared.sourceIdentity);
    assert.equal(state.field(nativeConditioning), null);
    assert.equal(state.doc, before.doc);
    assert.equal(state.field(lineEndings), before.field(lineEndings));
    assert.equal(state.selection, before.selection);
    assert.equal(state.field(editorFreshness), before.field(editorFreshness));
    assert.equal(readExactSource(state) === 'h\r\n"{one}"', true);
    assert.deepEqual([undoDepth(state), redoDepth(state)], history);
    fixture.owner.pointerStart();
    assert.equal(fixture.jobs.length, 1);
    assert.equal(fixture.state().field(semanticStructures).native, prepared);
    assert.equal(fixture.state().field(richProjection), projection);
  } finally {
    fixture.destroy();
  }
});

test('authoritative input composition fences queue/read/deferred/final boundaries and genuine release publishes populated effects', () => {
  for (const boundary of ['queue', 'read', 'deferred', 'final'] as const) {
    const fixture = refreshFixture('prompt', boundary === 'queue');
    try {
      if (boundary === 'queue') assert.equal(fixture.jobs.length, 0);
      else {
        if (boundary === 'read') fixture.compose(true);
        fixture.jobs[0]!.read();
        fixture.jobs[0]!.write();
        if (boundary === 'deferred') fixture.compose(true);
        if (boundary === 'final') fixture.reenter(() => fixture.compose(true), 1);
        fixture.task();
      }
      assert.equal(fixture.publications(), 0);
      assert.equal(fixture.state().field(semanticStructures).native, null);
      const count = fixture.jobs.length;
      fixture.owner.pointerStart();
      assert.equal(fixture.jobs.length, count);
      fixture.compose(false);
      fixture.owner.keyEnd();
      fixture.task();
      assert.equal(fixture.jobs.length, count + 1);
      fixture.jobs[count]!.read();
      fixture.jobs[count]!.write();
      fixture.task();
      const prepared = fixture.state().field(semanticStructures).native!;
      assert.equal(fixture.publications(), 1);
      assert.equal(prepared.scopes.scopes[0]!.document.emphasisSpans.length, 1);
      assert.ok(fixture.state().field(richProjection).all.size > 0);
      assert.equal(fixture.state().field(nativeConditioning) !== null, true);
    } finally {
      fixture.destroy();
    }
  }
});

test('waiting measurement supersession captures replacement source in the next native request while retired callbacks stay inert', () => {
  const fixture = refreshFixture();
  try {
    const retired = fixture.jobs[0]!;
    fixture.apply(
      fixture
        .state()
        .update(exactReplacement(fixture.state(), 0, fixture.state().doc.length, '(new:1.20)')),
    );
    assert.equal(fixture.jobs.length, 2);
    retired.read();
    retired.write();
    assert.equal(fixture.publications(), 0);
    const current = fixture.jobs[1]!;
    current.read();
    current.write();
    fixture.task();
    assert.equal(fixture.publications(), 1);
    assert.equal(
      fixture.state().field(semanticStructures).native!.scopes.scopes[0]!.document.emphasisSpans
        .length,
      1,
    );
    assert.equal(
      fixture.state().field(richProjection).sourceIdentity,
      fixture.state().field(editorFreshness).source,
    );
    assert.equal(fixture.jobs.length, 2);
  } finally {
    fixture.destroy();
  }
});
