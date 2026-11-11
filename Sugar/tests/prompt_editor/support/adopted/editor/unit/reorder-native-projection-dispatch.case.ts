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
/** Actual fixture dispatch forwards native plugin updates without inventing attachment measurements. */
import assert from 'node:assert/strict';
import test from 'node:test';
import { undoDepth, redoDepth } from '@codemirror/commands';
import { nativeFixture } from './reorder-native-projection-fixture.ts';
import { readExactSource } from '../../../../../../frontend/sugarprompt/editor/src/exact-source.ts';

for (const changed of [false, true])
  test(`native projection dispatch ${changed ? 'queues actual geometry' : 'keeps presentation measurement-free'}`, () => {
    const t = nativeFixture(),
      failures: unknown[] = [];
    try {
      const source = readExactSource(t.state),
        selection = t.state.selection,
        reads: string[] = [];
      t.f.observeMeasureReads((_value, owner) => reads.push(owner));
      assert.equal(t.f.jobs.length, 0);
      const publication = t.publish();
      assert.equal(publication.native, true);
      assert.equal(t.f.jobs.length, 0);
      assert.deepEqual(reads, []);
      assert.throws(() => t.f.drawLayer(), {
        message: 'fixture-attachment-request-required',
      });
      assert.equal(readExactSource(t.state), source);
      assert.equal(t.state.selection, selection);
      assert.equal(undoDepth(t.state), 0);
      assert.equal(redoDepth(t.state), 0);
      if (changed) {
        t.f.view.dispatch({
          changes: { from: t.state.doc.length, insert: 'x' },
        });
        assert.equal(t.f.jobs.length, 1);
        assert.deepEqual(reads, []);
        t.f.attachment.update({
          geometryChanged: false,
          viewportChanged: true,
        });
        assert.equal(t.f.jobs.length, 1);
        const frame = t.f.jobs.shift();
        assert.ok(frame);
        frame();
        assert.deepEqual(reads, ['layer']);
        assert.equal(t.f.jobs.length, 0);
        assert.equal(readExactSource(t.state), source + 'x');
        assert.equal(undoDepth(t.state), 1);
        assert.equal(redoDepth(t.state), 0);
      }
    } catch (cause) {
      failures.push(cause);
    }
    for (const release of [() => t.owner.destroy(), () => t.f.destroyLayer()])
      try {
        release();
      } catch (cause) {
        failures.push(cause);
      }
    if (failures.length === 1) throw failures[0];
    if (failures.length)
      throw new AggregateError(failures, 'native-projection-dispatch-proof-retirement');
  });
