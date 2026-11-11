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
/** Literal CSSOM delegates prove finite physical publication, protected closure and raw ordered failures independently of Scene semantics. */
import assert from 'node:assert/strict';
import test from 'node:test';
import { ReorderSlotPublication } from '../../../../../../frontend/sugarprompt/editor/src/reorder-slot-publication.ts';
import { motionPaintFixture } from './reorder-motion-paint-fixture.ts';

/** One real controlled native node belongs to exactly one finite physical publisher. */
function fixture() {
  const f = motionPaintFixture(),
    shell = f.document.createElement('span');
  f.editor.append(shell);
  const owner = new ReorderSlotPublication(
    shell as unknown as HTMLElement,
    f.editor as unknown as HTMLElement,
  );
  /** Retire all acquired native resources even if an earlier teardown delegate fails. */
  const destroy = (): void => {
    const failures: unknown[] = [];
    for (const release of [() => owner.retire(), () => shell.remove(), () => f.destroyLayer()])
      try {
        release();
      } catch (cause) {
        failures.push(cause);
      }
    if (failures.length === 1) throw failures[0];
    if (failures.length)
      throw new AggregateError(failures, 'publication-fixture-retirement-failed');
  };
  return { f, shell, owner, destroy };
}

test('construction, partial rectangle, whole reveal and close preserve authored descendant opacity', () => {
  const f = fixture(),
    root = f.f.document.createElement('span');
  f.shell.append(root);
  root.style.opacity = '0.73';
  const proofFailures: unknown[] = [];
  try {
    f.owner.initialize(() => true);
    assert.equal(f.shell.style.clip, 'rect(0px, 0px, 0px, 0px)');
    assert.equal(f.shell.style.getPropertyPriority('clip'), 'important');
    assert.equal(f.shell.style.position, 'absolute');
    assert.equal(f.shell.style.getPropertyPriority('position'), 'important');
    assert.equal(f.shell.style.opacity, '');
    assert.equal(f.shell.style.clipPath, '');
    f.owner.write(
      () => true,
      () => true,
      (current) => {
        assert.equal(f.owner.busy, true);
        assert.equal(current(), true);
        assert.equal(f.shell.style.clip, 'rect(0px, 0px, 0px, 0px)');
        root.style.visibility = 'visible';
      },
      { top: -2, right: 13, bottom: 17, left: 5 },
      () => assert.fail('unexpected-publication-failure'),
    );
    assert.equal(f.shell.style.clip, 'rect(-2px, 13px, 17px, 5px)');
    assert.equal(f.shell.style.getPropertyPriority('clip'), 'important');
    f.owner.write(
      () => true,
      () => true,
      () => {},
      null,
      () => assert.fail('unexpected-whole-failure'),
    );
    assert.equal(f.shell.style.clip, 'auto');
    f.shell.dataset.reorderMotionProgress = '1';
    f.shell.dataset.reorderMotionEndpoint = 'issued';
    const failures = f.owner.close(
      () => true,
      true,
      () => {
        assert.equal(f.owner.busy, true);
        assert.equal(f.shell.style.clip, 'rect(0px, 0px, 0px, 0px)');
        root.style.visibility = '';
        return [];
      },
    );
    assert.deepEqual(failures, []);
    assert.equal(f.owner.busy, false);
    assert.equal(root.style.opacity, '0.73');
    assert.equal(f.shell.style.opacity, '');
    assert.equal(f.shell.style.clipPath, '');
    assert.equal(f.shell.style.visibility, 'hidden');
    assert.equal(f.shell.dataset.reorderMotionProgress, undefined);
    assert.equal(f.shell.dataset.reorderMotionEndpoint, undefined);
  } catch (cause) {
    proofFailures.push(cause);
  }
  try {
    f.destroy();
  } catch (cause) {
    proofFailures.push(cause);
  }
  if (proofFailures.length === 1) throw proofFailures[0];
  if (proofFailures.length)
    throw new AggregateError(proofFailures, 'physical-publication-proof-retirement-failed');
});

test('close attempts pixels, attributes and roots and retains null/undefined in native order', () => {
  const f = fixture(),
    endpoint = new Error('endpoint-retirement');
  const proofFailures: unknown[] = [];
  try {
    f.owner.initialize(() => true);
    const set = f.shell.style.setProperty.bind(f.shell.style),
      attempts: number[] = [];
    f.shell.style.setProperty = (name, value, priority) => {
      set(name, value, priority);
      if (name === 'clip') {
        attempts.push(1);
        // eslint-disable-next-line @typescript-eslint/only-throw-error -- Inject raw host failures to prove original cause retention and cleanup.
        throw null;
      }
    };
    Object.defineProperty(f.shell, 'dataset', {
      value: new Proxy(
        {},
        {
          deleteProperty(_target, name) {
            if (name === 'reorderMotionProgress') {
              attempts.push(2);
              // eslint-disable-next-line @typescript-eslint/only-throw-error -- Inject raw host failures to prove original cause retention and cleanup.
              throw undefined;
            }
            attempts.push(3);
            throw endpoint;
          },
        },
      ),
    });
    const root = new Error('root-retirement');
    const failures = f.owner.close(
      () => true,
      true,
      () => {
        attempts.push(4);
        return [root];
      },
    );
    assert.deepEqual(attempts, [1, 2, 3, 4]);
    assert.deepEqual(failures, [null, undefined, endpoint, root]);
    assert.equal(f.owner.busy, false);
    assert.equal(f.shell.style.clip, 'rect(0px, 0px, 0px, 0px)');
  } catch (cause) {
    proofFailures.push(cause);
  }
  try {
    f.destroy();
  } catch (cause) {
    proofFailures.push(cause);
  }
  if (proofFailures.length === 1) throw proofFailures[0];
  if (proofFailures.length)
    throw new AggregateError(proofFailures, 'physical-publication-proof-retirement-failed');
});

test('a resumed old clip setter closes its captured node without writing a replacement publisher', () => {
  const old = fixture(),
    fresh = fixture();
  const proofFailures: unknown[] = [];
  try {
    old.owner.initialize(() => true);
    fresh.owner.initialize(() => true);
    const set = old.shell.style.setProperty.bind(old.shell.style);
    let entered = false;
    old.shell.style.setProperty = (name, value, priority) => {
      if (!entered && name === 'clip' && value === 'auto') {
        entered = true;
        assert.equal(old.owner.busy, true);
        assert.deepEqual(
          old.owner.close(
            () => true,
            false,
            () => [],
          ),
          [],
        );
        assert.equal(old.owner.busy, true);
        old.owner.retire();
        old.shell.remove();
      }
      set(name, value, priority);
    };
    old.owner.write(
      () => true,
      () => true,
      () => {},
      null,
      () => assert.fail('unexpected-late-write-error'),
    );
    assert.equal(entered, true);
    assert.equal(old.shell.isConnected, false);
    assert.equal(old.shell.style.clip, 'rect(0px, 0px, 0px, 0px)');
    assert.equal(fresh.shell.style.clip, 'rect(0px, 0px, 0px, 0px)');
    assert.equal(fresh.owner.busy, false);
  } catch (cause) {
    proofFailures.push(cause);
  }
  try {
    const failures: unknown[] = [];
    for (const release of [old.destroy, fresh.destroy])
      try {
        release();
      } catch (cause) {
        failures.push(cause);
      }
    if (failures.length === 1) throw failures[0];
    if (failures.length) throw new AggregateError(failures, 'paired-publication-retirement-failed');
  } catch (cause) {
    proofFailures.push(cause);
  }
  if (proofFailures.length === 1) throw proofFailures[0];
  if (proofFailures.length)
    throw new AggregateError(proofFailures, 'physical-publication-proof-retirement-failed');
});

test('publication error precedes closure and terminal-retirement failures without losing raw values', () => {
  const f = fixture(),
    terminal = new Error('terminal-retirement');
  const proofFailures: unknown[] = [];
  try {
    f.owner.initialize(() => true);
    f.shell.style.setProperty = () => {
      // eslint-disable-next-line @typescript-eslint/only-throw-error -- Inject raw host failures to prove original cause retention and cleanup.
      throw undefined;
    };
    assert.throws(
      () =>
        f.owner.write(
          () => true,
          () => true,
          () => {
            // eslint-disable-next-line @typescript-eslint/only-throw-error -- Inject raw host failures to prove original cause retention and cleanup.
            throw null;
          },
          null,
          () => {
            throw terminal;
          },
        ),
      (cause) => {
        assert.ok(cause instanceof AggregateError);
        assert.deepEqual(cause.errors, [null, undefined, terminal]);
        return true;
      },
    );
    assert.equal(f.owner.busy, false);
  } catch (cause) {
    proofFailures.push(cause);
  }
  try {
    f.destroy();
  } catch (cause) {
    proofFailures.push(cause);
  }
  if (proofFailures.length === 1) throw proofFailures[0];
  if (proofFailures.length)
    throw new AggregateError(proofFailures, 'physical-publication-proof-retirement-failed');
});

for (const raw of [null, undefined])
  test(`initial closure retains raw ${String(raw)} and releases its native reservation`, () => {
    const f = fixture();
    const proofFailures: unknown[] = [];
    try {
      const set = f.shell.style.setProperty.bind(f.shell.style);
      f.shell.style.setProperty = (name, value, priority) => {
        // eslint-disable-next-line @typescript-eslint/only-throw-error -- Inject raw host failures to prove original cause retention and cleanup.
        if (name === 'clip') throw raw;
        set(name, value, priority);
      };
      assert.throws(
        () => f.owner.initialize(() => true),
        (cause) => cause === raw,
      );
      assert.equal(f.owner.busy, false);
    } catch (cause) {
      proofFailures.push(cause);
    }
    try {
      f.destroy();
    } catch (cause) {
      proofFailures.push(cause);
    }
    if (proofFailures.length === 1) throw proofFailures[0];
    if (proofFailures.length)
      throw new AggregateError(proofFailures, 'physical-publication-proof-retirement-failed');
  });
