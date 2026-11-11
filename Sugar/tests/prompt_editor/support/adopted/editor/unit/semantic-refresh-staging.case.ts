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
/** Bound large-source preparation tasks while preserving publication and native input authority. */
import assert from 'node:assert/strict';
import test from 'node:test';
import { undoDepth, redoDepth } from '@codemirror/commands';
import { semanticStructures } from '../../../../../../frontend/sugarprompt/editor/src/structure.ts';
import { richProjection } from '../../../../../../frontend/sugarprompt/editor/src/rich-projection.ts';
import {
  exactReplacement,
  readExactSource,
} from '../../../../../../frontend/sugarprompt/editor/src/exact-source.ts';
import { refreshFixture } from './semantic-refresh-fixture.ts';

const source = 'plain, (soft eyes:1.2), {colors}, tail, '.repeat(300);

test('large preparation yields between canonical phases and publishes once without editing source or history', () => {
  const f = refreshFixture('prompt', false, source);
  try {
    const before = f.state(),
      history = [undoDepth(before), redoDepth(before)];
    f.jobs[0]!.read();
    f.jobs[0]!.write();
    f.task();
    assert.equal(f.publications(), 0);
    assert.equal(f.state().field(semanticStructures).native, null);
    assert.equal(f.backgroundTasks(), 1);
    let steps = 1;
    while (f.backgroundTasks() && steps++ < 20) f.task();
    assert.ok(steps > 3 && steps < 20);
    assert.equal(f.publications(), 1);
    const after = f.state(),
      prepared = after.field(semanticStructures).native!;
    assert.equal(prepared.scopes.scopes[0]!.document.sourceText, source);
    assert.equal(after.field(richProjection).sourceIdentity, prepared.sourceIdentity);
    assert.ok(after.field(richProjection).all.size > 0);
    assert.equal(after.doc, before.doc);
    assert.equal(after.selection, before.selection);
    assert.deepEqual([undoDepth(after), redoDepth(after)], history);
    assert.equal(readExactSource(after), source);
  } finally {
    f.destroy();
  }
});

for (const boundary of [1, 3])
  for (const retirement of ['source', 'held', 'composition', 'permission', 'dispose'] as const)
    test(`large preparation stops after ${boundary} phases when ${retirement} retires its input`, () => {
      const f = refreshFixture('prompt', false, source);
      try {
        f.jobs[0]!.read();
        f.jobs[0]!.write();
        for (let i = 0; i < boundary; i++) f.task();
        assert.equal(f.publications(), 0);
        if (retirement === 'source')
          f.apply(f.state().update(exactReplacement(f.state(), 0, 0, 'new, ')));
        if (retirement === 'held') f.hold(true);
        if (retirement === 'composition') f.compose(true);
        if (retirement === 'permission') f.reconfigure('permission');
        if (retirement === 'dispose') f.owner.destroy();
        while (f.backgroundTasks()) f.task();
        assert.equal(f.publications(), 0);
        assert.equal(f.state().field(semanticStructures).native, null);
        if (retirement === 'dispose') return;
        if (retirement === 'held') f.hold(false);
        if (retirement === 'composition') f.compose(false);
        f.owner.keyEnd();
        f.task();
        const job = f.jobs.at(-1)!;
        assert.notEqual(job, f.jobs[0]);
        job.read();
        job.write();
        let steps = 0;
        while (f.backgroundTasks() && steps++ < 20) f.task();
        assert.equal(f.publications(), 1);
        assert.equal(
          readExactSource(f.state()),
          retirement === 'source' ? 'new, ' + source : source,
        );
      } finally {
        f.destroy();
      }
    });
