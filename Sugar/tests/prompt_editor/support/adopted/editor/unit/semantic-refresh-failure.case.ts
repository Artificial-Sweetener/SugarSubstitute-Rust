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
/** Failed or superseded measurement reservations retain observable causes and current followup. */
import assert from 'node:assert/strict';
import test from 'node:test';
import { EditorSelection } from '@codemirror/state';
import { semanticStructures } from '../../../../../../frontend/sugarprompt/editor/src/structure.ts';
import { exactReplacement } from '../../../../../../frontend/sugarprompt/editor/src/exact-source.ts';
import { nativeConditioning } from '../../../../../../frontend/sugarprompt/editor/src/conditioning-state.ts';
import { richProjection } from '../../../../../../frontend/sugarprompt/editor/src/rich-projection.ts';
import { invalidateCommandContext } from '../../../../../../frontend/sugarprompt/editor/src/freshness.ts';
import { refreshFixture } from './semantic-refresh-fixture.ts';

test('input observation reentry cannot publish retired source or swallow an invariant cause', () => {
  const fixture = refreshFixture();
  try {
    fixture.jobs[0]!.read();
    fixture.jobs[0]!.write();
    fixture.reenter(
      () => fixture.apply(fixture.state().update(exactReplacement(fixture.state(), 0, 0, 'a'))),
      1,
    );
    fixture.task();
    assert.equal(fixture.publications(), 0);
    fixture.jobs[1]!.read();
    fixture.jobs[1]!.write();
    const cause = new Error('fixture-invariant');
    fixture.reenter(() => {
      throw cause;
    });
    assert.throws(
      () => fixture.task(),
      (error: unknown) => error === cause,
    );
  } finally {
    fixture.destroy();
  }
});

test('composition-observation reentry fences actual state/generation/source/permission/disposal after both preparation transitions', () => {
  for (const boundary of ['read', 'deferred', 'final'] as const)
    for (const retirement of [
      'source',
      'selection',
      'context',
      'permission',
      'profile',
      'kind-return',
      'input',
      'dispose',
      'composition',
    ] as const) {
      const fixture = refreshFixture();
      try {
        /** Retire actual owner inputs inside its authoritative observation, without browser certification. */
        const retire = (): void => {
          if (retirement === 'dispose') fixture.owner.destroy();
          else if (retirement === 'composition') fixture.compose(true);
          else if (
            retirement === 'permission' ||
            retirement === 'profile' ||
            retirement === 'kind-return' ||
            retirement === 'input'
          )
            fixture.reconfigure(retirement);
          else
            fixture.apply(
              fixture
                .state()
                .update(
                  retirement === 'source'
                    ? exactReplacement(fixture.state(), 0, 0, 'a')
                    : retirement === 'selection'
                      ? { selection: EditorSelection.single(4, 1) }
                      : { effects: invalidateCommandContext.of(null) },
                ),
            );
        };
        if (boundary === 'read') fixture.reenterComposition(retire);
        fixture.jobs[0]!.read();
        fixture.jobs[0]!.write();
        if (boundary !== 'read') fixture.reenterComposition(retire, boundary === 'final' ? 2 : 0);
        fixture.task();
        assert.equal(fixture.publications(), 0);
        assert.equal(fixture.state().field(semanticStructures).native, null);
        if (retirement !== 'dispose') {
          if (retirement === 'composition') {
            fixture.compose(false);
            fixture.owner.keyEnd();
            fixture.task();
          }
          const job = fixture.jobs.at(-1)!;
          assert.notEqual(job, fixture.jobs[0]);
          job.read();
          job.write();
          fixture.task();
          assert.equal(fixture.publications(), 1, `${boundary}:${retirement}`);
        }
      } finally {
        fixture.destroy();
      }
    }
});

test('queue observation reentry schedules only current followup and disposal retires its finite microtask', () => {
  for (const dispose of [false, true]) {
    const fixture = refreshFixture('prompt', true);
    try {
      fixture.compose(false);
      fixture.reenterComposition(() =>
        fixture.apply(fixture.state().update(exactReplacement(fixture.state(), 0, 0, 'a'))),
      );
      fixture.owner.keyEnd();
      fixture.task();
      assert.equal(fixture.jobs.length, 0);
      assert.equal(fixture.publications(), 0);
      if (dispose) fixture.owner.destroy();
      fixture.task();
      assert.equal(fixture.jobs.length, dispose ? 0 : 1);
      if (!dispose) {
        fixture.jobs[0]!.read();
        fixture.jobs[0]!.write();
        fixture.task();
        assert.equal(fixture.publications(), 1);
      }
    } finally {
      fixture.destroy();
    }
  }
});

test('native composition getter reentry and authoritative observation failures preserve publication fences and original causes', () => {
  for (const retirement of ['composition', 'source', 'dispose', 'cause'] as const) {
    const fixture = refreshFixture();
    try {
      fixture.jobs[0]!.read();
      fixture.jobs[0]!.write();
      const cause = new Error('fixture-observation-invariant');
      /** Challenge the final native getter after the deferred readiness observations. */
      const retire = (): void => {
        if (retirement === 'cause') throw cause;
        if (retirement === 'composition') fixture.compose(true);
        if (retirement === 'dispose') fixture.owner.destroy();
        if (retirement === 'source')
          fixture.apply(fixture.state().update(exactReplacement(fixture.state(), 0, 0, 'a')));
      };
      fixture.reenterNativeComposition(retire, 2);
      if (retirement === 'cause')
        assert.throws(
          () => fixture.task(),
          (error: unknown) => error === cause,
        );
      else fixture.task();
      assert.equal(fixture.publications(), 0);
      assert.equal(fixture.state().field(semanticStructures).native, null);
      if (retirement !== 'dispose') {
        fixture.compose(false);
        fixture.owner.keyEnd();
        fixture.task();
        const job = fixture.jobs.at(-1)!;
        job.read();
        job.write();
        fixture.task();
        assert.equal(fixture.publications(), 1);
      }
    } finally {
      fixture.destroy();
    }
  }
});

test('the last native getter cannot begin input-owner composition after an earlier false observation', () => {
  const fixture = refreshFixture();
  try {
    fixture.jobs[0]!.read();
    fixture.jobs[0]!.write();
    fixture.reenterNativeComposition(() => fixture.compose(true), 3);
    fixture.task();
    assert.equal(fixture.publications(), 0);
    assert.equal(fixture.state().field(semanticStructures).native, null);
    fixture.compose(false);
    fixture.owner.keyEnd();
    fixture.task();
    fixture.jobs[1]!.read();
    fixture.jobs[1]!.write();
    fixture.task();
    assert.equal(fixture.publications(), 1);
  } finally {
    fixture.destroy();
  }
});

test('failed measured input observations release only their reservation and genuine release publishes populated output', () => {
  for (const observation of ['held', 'composition', 'native'] as const) {
    const fixture = refreshFixture();
    try {
      const cause = new Error('fixture-measured-observation-invariant');
      /** Failure happens in the actual read callback; CodeMirror omits its write phase. */
      const fail = (): void => {
        throw cause;
      };
      if (observation === 'held') fixture.reenter(fail);
      if (observation === 'composition') fixture.reenterComposition(fail);
      if (observation === 'native') fixture.reenterNativeComposition(fail);
      assert.throws(
        () => fixture.jobs[0]!.read(),
        (error: unknown) => error === cause,
      );
      assert.equal(fixture.publications(), 0);
      assert.equal(fixture.state().field(semanticStructures).native, null);
      fixture.owner.keyEnd();
      fixture.task();
      assert.equal(fixture.jobs.length, 2);
      fixture.jobs[0]!.read();
      fixture.jobs[0]!.write();
      fixture.owner.pointerStart();
      assert.equal(fixture.jobs.length, 2);
      fixture.jobs[1]!.read();
      fixture.jobs[1]!.write();
      fixture.task();
      assert.equal(fixture.publications(), 1);
      const prepared = fixture.state().field(semanticStructures).native!;
      assert.equal(prepared.scopes.scopes[0]!.document.emphasisSpans.length, 1);
      assert.ok(fixture.state().field(richProjection).all.size > 0);
      assert.notEqual(fixture.state().field(nativeConditioning), null);
    } finally {
      fixture.destroy();
    }
  }
});
