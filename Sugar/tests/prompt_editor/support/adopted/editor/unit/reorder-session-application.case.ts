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
/** Pointer proposals retain the exact authored hold vacancy while genuine destinations use Core insertion. */
import assert from 'node:assert/strict';
import test from 'node:test';
import {
  hideReorderChip,
  dropReorderChip,
  valueReorderVacancy,
  valueReorderLayout,
  serializeValueReorder,
  type ReorderDropTarget,
} from '../../../../../../frontend/sugarprompt/core/src/index.ts';
import { captureReorder, reorderState } from '../transaction/reorder-fixture.ts';
import {
  prepareSessionReorderApplication,
  createReorderApplicationHold,
  type ReorderApplicationInput,
} from '../../../../../../frontend/sugarprompt/editor/src/reorder-session-application.ts';

/** A genuine capture supplies immutable Core operands; only Session publication is observed. */
function application(source = 'a,  b, c') {
  const state = reorderState(source).state,
    capture = captureReorder(state),
    order = capture.prepared.initial;
  const held = order.order[0]!;
  const hidden = hideReorderChip(capture.prepared, order, held);
  assert.ok(hidden);
  const target = valueReorderVacancy(order, hidden, held);
  assert.ok(target);
  const input: ReorderApplicationInput = {
    token: {},
    capture,
    order,
    held,
    hidden,
    hold: createReorderApplicationHold(order),
    pointer: 1,
    coordinates: { x: 15, y: 20 },
  };
  let publications = 0;
  return {
    input,
    hidden,
    target,
    get publications() {
      return publications;
    },
    prepare(current = () => true, pointer = true, destination: ReorderDropTarget = target) {
      return prepareSessionReorderApplication(input, hidden, destination, pointer, current, () => {
        publications++;
        return true;
      });
    },
  };
}

test('the authored pointer vacancy retains its exact source carriers and layout without publication', () => {
  const f = application(),
    original = f.input.order,
    result = f.prepare();
  assert.equal(result.kind, 'unchanged');
  assert.equal(result.kind === 'unchanged' && result.current(), true);
  assert.equal(f.input.order === original, true);
  assert.equal(f.input.order.slots[0]?.text === ',  ', true);
  assert.equal(
    serializeValueReorder(f.input.capture.prepared, f.input.order)!.text === 'a,  b, c',
    true,
  );
  assert.deepEqual(valueReorderLayout(f.input.order), valueReorderLayout(original));
  assert.equal(f.publications, 0);
});

test('returning after a real proposal restores the exact hold-origin state once', () => {
  const f = application(),
    original = f.input.order;
  const away = f.prepare(() => true, true, {
    kind: 'row',
    row: 0,
    insertion: 2,
  });
  assert.equal(away.kind === 'changed' && away.apply(), true);
  assert.deepEqual(f.input.order.order, [1, 2, 0]);
  const back = f.prepare();
  assert.equal(back.kind === 'changed' && back.apply(), true);
  assert.equal(f.input.order === original, true);
  assert.equal(
    serializeValueReorder(f.input.capture.prepared, f.input.order)!.text === 'a,  b, c',
    true,
  );
  assert.equal(f.publications, 2);
  assert.equal(f.prepare().kind, 'unchanged');
  assert.equal(f.publications, 2);
});

test('a new hold retains its own authored vacancy after an earlier committed proposal', () => {
  const f = application();
  const away = f.prepare(() => true, true, {
    kind: 'row',
    row: 0,
    insertion: 2,
  });
  assert.equal(away.kind === 'changed' && away.apply(), true);
  const origin = f.input.order,
    hidden = hideReorderChip(f.input.capture.prepared, origin, f.input.held);
  assert.ok(hidden);
  const target = valueReorderVacancy(origin, hidden, f.input.held);
  assert.ok(target);
  const input: ReorderApplicationInput = {
    ...f.input,
    hidden,
    hold: createReorderApplicationHold(origin),
  };
  const result = prepareSessionReorderApplication(
    input,
    hidden,
    target,
    true,
    () => true,
    () => {
      throw new Error('unexpected-vacancy-publication');
    },
  );
  assert.equal(result.kind, 'unchanged');
  assert.equal(input.order === origin, true);
});

test('a different row destination remains a genuine move even with the same chip IDs', () => {
  const f = application('a\nb, c'),
    original = f.input.order;
  assert.equal(valueReorderLayout(original).rows.length, 2);
  assert.notEqual(f.target.kind, 'row');
  const result = f.prepare(() => true, true, {
    kind: 'row',
    row: 0,
    insertion: 0,
  });
  assert.equal(result.kind === 'changed' && result.apply(), true);
  assert.deepEqual(f.input.order.order, original.order);
  assert.equal(f.input.order === original, false);
  assert.equal(valueReorderLayout(f.input.order).rows.length, 1);
  assert.equal(
    serializeValueReorder(f.input.capture.prepared, f.input.order)!.text === 'a\nb, c',
    false,
  );
  assert.equal(f.publications, 1);
});
test('prepared Core result rejects replacement hold and post-current operand changes', () => {
  const f = application(),
    outcome = f.prepare(() => true, true, {
      kind: 'row',
      row: 0,
      insertion: 2,
    });
  assert.equal(outcome.kind, 'changed');
  Object.defineProperty(f.input, 'hold', {
    value: createReorderApplicationHold(f.input.order),
  });
  assert.equal(outcome.kind === 'changed' && outcome.apply(), false);
  assert.equal(f.publications, 0);
  const g = application();
  const stale = g.prepare(() => {
    g.input.hidden = null;
    return true;
  });
  assert.equal(stale.kind, 'unavailable');
  assert.equal(g.publications, 0);
});

test('keyboard completion consumes the same Core result and retires hidden operand', () => {
  const f = application(),
    result = f.prepare(() => true, false);
  assert.equal(result.kind === 'changed' && result.apply(), true);
  assert.equal(f.input.hidden, null);
  assert.equal(f.publications, 1);
});

test('publication preserves its raw failure after consuming the exact prepared result', () => {
  const f = application();
  const outcome = prepareSessionReorderApplication(
    f.input,
    f.hidden,
    { kind: 'row', row: 0, insertion: 2 },
    true,
    () => true,
    () => {
      // eslint-disable-next-line @typescript-eslint/only-throw-error -- Inject raw host failures to prove original cause retention and cleanup.
      throw undefined;
    },
  );
  assert.equal(outcome.kind, 'changed');
  let caught = false;
  try {
    if (outcome.kind === 'changed') outcome.apply();
  } catch (cause) {
    caught = true;
    assert.equal(cause, undefined);
  }
  assert.equal(caught, true);
  assert.equal(outcome.kind === 'changed' && outcome.apply(), false);
});

test('one actual hold reuses its frozen Core target and avoids derivation under exact hidden and held operands', () => {
  const { state } = reorderState('a,  b, c'),
    capture = captureReorder(state),
    origin = capture.prepared.initial,
    hidden = hideReorderChip(capture.prepared, origin, 0),
    failure = new Error('unexpected-cold-vacancy-read');
  assert.ok(hidden);
  let readable = true;
  const watched = new Proxy(origin, {
      /** Observe the genuine immutable operand without substituting any semantic value. */
      get(target, key): unknown {
        if (key === 'order' && !readable) throw failure;
        return Reflect.get(target, key);
      },
    }),
    hold = createReorderApplicationHold(watched),
    target = hold.vacancy(hidden, 0);
  assert.deepEqual(target, { kind: 'row', row: 0, insertion: 0 });
  assert.equal(Object.isFrozen(hold), true);
  assert.equal(Object.isFrozen(target), true);
  readable = false;
  assert.equal(hold.vacancy(hidden, 0) === target, true);
  const replacement = hideReorderChip(capture.prepared, origin, 0);
  assert.ok(replacement);
  assert.equal(replacement === hidden, false);
  assert.throws(
    () => hold.vacancy(replacement, 0),
    (cause) => cause === failure,
  );
  readable = true;
  const next = hold.vacancy(replacement, 0);
  assert.deepEqual(next, { kind: 'row', row: 0, insertion: 0 });
  assert.equal(next === target, false);
  readable = false;
  assert.equal(hold.vacancy(replacement, 0) === next, true);
  assert.throws(
    () => hold.vacancy(replacement, 99),
    (cause) => cause === failure,
  );
  readable = true;
  assert.equal(hold.vacancy(replacement, 99), null);
  readable = false;
  assert.equal(hold.vacancy(replacement, 99), null);
  assert.throws(
    () => hold.vacancy(replacement, 0),
    (cause) => cause === failure,
  );
  const fresh = createReorderApplicationHold(watched);
  assert.throws(
    () => fresh.vacancy(replacement, 99),
    (cause) => cause === failure,
  );
  readable = true;
  assert.equal(fresh.vacancy(replacement, 0) === next, false);
  assert.equal(serializeValueReorder(capture.prepared, origin)!.text === 'a,  b, c', true);
});

test('fresh source-bound holds derive their own actual vacancy and preserve raw derivation failures', () => {
  const first = application(),
    { state } = reorderState('a\nb, c'),
    capture = captureReorder(state),
    origin = capture.prepared.initial,
    hidden = hideReorderChip(capture.prepared, origin, 0);
  assert.ok(hidden);
  assert.equal(first.input.hold!.vacancy(first.hidden, 0)?.kind, 'row');
  const hold = createReorderApplicationHold(origin);
  assert.equal(hold.vacancy(hidden, 0)?.kind, 'gap');
  const broken = createReorderApplicationHold(
    new Proxy(origin, {
      /** A raw unexpected semantic-read failure must remain observable. */
      get(target, key): unknown {
        // eslint-disable-next-line @typescript-eslint/only-throw-error -- Inject raw host failures to prove original cause retention and cleanup.
        if (key === 'order') throw undefined;
        return Reflect.get(target, key);
      },
    }),
  );
  let caught = false;
  try {
    broken.vacancy(hidden, 0);
  } catch (cause) {
    caught = true;
    assert.equal(cause, undefined);
  }
  assert.equal(caught, true);
});

test('one hold reuses actual Core drop output without rereading source and replaces its single entry for changed operands', () => {
  const f = application(),
    prepared = f.input.capture.prepared,
    hold = f.input.hold!,
    failure = new Error('unexpected-cold-drop-read');
  let readable = true;
  const watched = new Proxy(prepared, {
    /** A warm hit must not enter Core's genuine source validation or transformation again. */
    get(target, key): unknown {
      if (!readable) throw failure;
      return Reflect.get(target, key);
    },
  });
  const target = { kind: 'row' as const, row: 0, insertion: 1 },
    first = hold.drop(watched, f.hidden, f.input.held, target);
  assert.ok(first);
  assert.equal(Object.isFrozen(first), true);
  assert.deepEqual(first, dropReorderChip(prepared, f.hidden, 0, target));
  assert.deepEqual(first.order, [1, 0, 2]);
  readable = false;
  assert.equal(hold.drop(watched, f.hidden, 0, { ...target }), first);
  target.insertion = 2;
  assert.throws(
    () => hold.drop(watched, f.hidden, 0, target),
    (cause) => cause === failure,
  );
  assert.equal(hold.drop(watched, f.hidden, 0, { ...target, insertion: 1 }), first);
  readable = true;
  const next = hold.drop(watched, f.hidden, 0, target);
  assert.ok(next);
  assert.notEqual(next, first);
  assert.deepEqual(next.order, [1, 2, 0]);
  readable = false;
  assert.equal(hold.drop(watched, f.hidden, 0, { ...target }), next);
  const hidden = hideReorderChip(prepared, f.input.order, 0);
  assert.ok(hidden);
  assert.notEqual(hidden, f.hidden);
  for (const cold of [
    () => hold.drop(new Proxy(watched, {}), f.hidden, 0, target),
    () => hold.drop(watched, hidden, 0, target),
    () => hold.drop(watched, f.hidden, 1, target),
    () => hold.drop(watched, f.hidden, 0, { ...target, insertion: 1 }),
    () => createReorderApplicationHold(f.input.order).drop(watched, f.hidden, 0, target),
  ]) {
    assert.throws(cold, (cause) => cause === failure);
  }
  readable = true;
  assert.equal(hold.drop(watched, f.hidden, 0, { ...target, insertion: 99 }), null);
  readable = false;
  assert.equal(hold.drop(watched, f.hidden, 0, { ...target, insertion: 99 }), null);
  assert.throws(
    () => hold.drop(watched, f.hidden, 0, target),
    (cause) => cause === failure,
  );
});

test('pointer preparation borrows the cached Core result with unchanged admission while keyboard preparation remains direct', () => {
  const f = application(),
    prepared = f.input.capture.prepared,
    failure = new Error('controlled-direct-core-drop-read');
  let readable = true,
    publications = 0;
  const watched = new Proxy(prepared, {
    /** Only a warmed pointer hold may avoid the original Core read boundary. */
    get(target, key): unknown {
      if (!readable) throw failure;
      return Reflect.get(target, key);
    },
  });
  const input: ReorderApplicationInput = {
    ...f.input,
    capture: { ...f.input.capture, prepared: watched },
  };
  const target = { kind: 'row' as const, row: 0, insertion: 2 },
    actual = input.hold!.drop(watched, f.hidden, input.held, target);
  assert.ok(actual);
  readable = false;
  /** The real adapter retains its own consumed/currentness and publication behavior around reused Core output. */
  const prepare = (pointer: boolean, current = () => true) =>
    prepareSessionReorderApplication(input, f.hidden, target, pointer, current, () => {
      publications++;
      return true;
    });
  const changed = prepare(true);
  assert.equal(changed.kind === 'changed' && changed.apply(), true);
  assert.equal(input.order, actual);
  assert.equal(changed.kind === 'changed' && changed.apply(), false);
  assert.equal(prepare(true).kind, 'unchanged');
  assert.equal(publications, 1);
  assert.equal(prepare(true, () => false).kind, 'unavailable');
  assert.equal(
    prepare(true, () => {
      input.hidden = null;
      return true;
    }).kind,
    'unavailable',
  );
  input.hidden = f.hidden;
  let admitted = false;
  assert.equal(
    prepare(true, () => {
      if (admitted) input.hidden = null;
      admitted = true;
      return true;
    }).kind,
    'unavailable',
  );
  input.hidden = f.hidden;
  assert.throws(
    () => prepare(false),
    (cause) => cause === failure,
  );
  assert.equal(publications, 1);
  assert.equal(input.order, actual);
  assert.equal(f.input.order, prepared.initial);
});
