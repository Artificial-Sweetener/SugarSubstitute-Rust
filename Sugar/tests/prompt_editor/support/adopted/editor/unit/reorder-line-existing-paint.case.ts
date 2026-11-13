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
/** Existing-output admission proves exact multiplicities and no hidden cold construction. */
import assert from 'node:assert/strict';
import test from 'node:test';
import { ReorderLineLayout } from '../../../../../../frontend/sugarprompt/editor/src/reorder-line-layout.ts';

/** Literal native nodes retain observable identity and independently owned removal. */
function output() {
  return { node: { remove() {} } as unknown as HTMLElement, destroy() {} };
}

test('complete multiset admits borrowed and idle copies and reborrows their actual original nodes', () => {
  const owner = new ReorderLineLayout(),
    a = output(),
    b = output(),
    first = owner.borrow('same', () => a),
    second = owner.borrow('same', () => b);
  second.release();
  const actual: HTMLElement[] = [];
  assert.equal(
    owner.reconcilePrepared(
      ['same', 'same'],
      () => false,
      () => {
        first.release();
        for (let i = 0; i < 2; i++)
          actual.push(
            owner.borrow('same', () => {
              throw new Error('unexpected-reconstruction');
            }).node,
          );
      },
    ),
    true,
  );
  assert.deepEqual(new Set(actual), new Set([a.node, b.node]));
  owner.destroy();
});

test('missing repeated-key occurrence refuses before touching currently borrowed output', () => {
  const owner = new ReorderLineLayout(),
    entry = output(),
    lease = owner.borrow('same', () => entry);
  let invoked = false;
  assert.equal(
    owner.reconcilePrepared(
      ['same', 'same'],
      () => false,
      () => {
        invoked = true;
        lease.release();
      },
    ),
    false,
  );
  assert.equal(invoked, false);
  assert.equal(lease.node, entry.node);
  assert.equal(
    owner.reconcilePrepared(
      ['same', 'missing'],
      () => false,
      () => {
        invoked = true;
      },
    ),
    false,
  );
  assert.equal(invoked, false);
  owner.destroy();
});

test('admitted warm scope rejects a mismatched borrow without invoking its constructor', () => {
  const owner = new ReorderLineLayout(),
    lease = owner.borrow('same', output);
  let invoked = false;
  assert.throws(
    () =>
      owner.reconcilePrepared(
        ['same'],
        () => false,
        () => {
          lease.release();
          owner.borrow('different', () => {
            invoked = true;
            return output();
          });
        },
      ),
    { message: 'reorder-line-existing-paint-missing' },
  );
  assert.equal(invoked, false);
  owner.destroy();
});

test('missing repeated fragments reserve exact creation occurrences before external factories', () => {
  const owner = new ReorderLineLayout(),
    original = output(),
    first = owner.borrow('plain', () => original),
    created = [output(), output()],
    actual: HTMLElement[] = [];
  let factories = 0;
  assert.equal(
    owner.reconcilePrepared(
      ['plain', 'plain', 'plain'],
      (key) => key === 'plain',
      () => {
        first.release();
        actual.push(owner.borrow('plain', () => assert.fail('existing-fragment-rebuilt')).node);
        actual.push(
          owner.borrow('plain', () => {
            factories++;
            assert.equal(
              owner.reconcilePrepared(
                ['extra'],
                () => true,
                () => assert.fail('nested-budget-reset'),
              ),
              false,
            );
            actual.push(
              owner.borrow('plain', () => {
                factories++;
                return created[1]!;
              }).node,
            );
            assert.throws(() => owner.borrow('plain', () => assert.fail('over-budget-factory')), {
              message: 'reorder-line-existing-paint-missing',
            });
            assert.throws(() => owner.borrow('wrong', () => assert.fail('wrong-key-factory')), {
              message: 'reorder-line-existing-paint-missing',
            });
            return created[0]!;
          }).node,
        );
      },
    ),
    true,
  );
  assert.equal(factories, 2);
  assert.deepEqual(
    new Set(actual),
    new Set([original.node, ...created.map((value) => value.node)]),
  );
  owner.destroy();
});

test('late refused classification cannot mutate or allocate earlier admitted missing fragments', () => {
  const owner = new ReorderLineLayout(),
    original = output(),
    held = owner.borrow('original', () => original),
    classified: string[] = [];
  let acted = false;
  assert.equal(
    owner.reconcilePrepared(
      ['original', 'plain', 'projected'],
      (key) => {
        classified.push(key);
        assert.throws(() => owner.borrow('plain', () => assert.fail('factory-during-preflight')), {
          message: 'reorder-line-existing-paint-missing',
        });
        return key === 'plain';
      },
      () => {
        acted = true;
        held.release();
      },
    ),
    false,
  );
  assert.deepEqual(classified, ['plain', 'projected']);
  assert.equal(acted, false);
  assert.equal(held.node, original.node);
  assert.equal(
    owner.reconcilePrepared(
      ['original'],
      () => assert.fail('available-key-classified'),
      () => {},
    ),
    true,
  );
  owner.destroy();
});

for (const raw of [null, undefined])
  test(`raw missing factory failure retires its exact budget: ${String(raw)}`, () => {
    const owner = new ReorderLineLayout();
    let caught = false;
    try {
      owner.reconcilePrepared(
        ['plain'],
        () => true,
        () => {
          owner.borrow('plain', () => {
            // eslint-disable-next-line @typescript-eslint/only-throw-error -- Inject raw host failures to prove original cause retention and cleanup.
            throw raw;
          });
        },
      );
    } catch (cause) {
      caught = true;
      assert.equal(cause, raw);
    }
    assert.equal(caught, true);
    assert.equal(
      owner.reconcilePrepared(
        ['plain'],
        () => false,
        () => assert.fail('old-budget-survived'),
      ),
      false,
    );
    owner.destroy();
  });

test('classification retirement is finite and cannot publish its prepared action', () => {
  const owner = new ReorderLineLayout();
  let destroyed = 0;
  owner.borrow('old', () => ({
    node: output().node,
    destroy() {
      destroyed++;
    },
  }));
  assert.equal(
    owner.reconcilePrepared(
      ['plain'],
      () => {
        owner.destroy();
        return true;
      },
      () => assert.fail('retired-action'),
    ),
    false,
  );
  assert.equal(destroyed, 1);
  assert.equal(
    owner.reconcilePrepared(
      ['plain'],
      () => true,
      () => assert.fail('disposed-action'),
    ),
    false,
  );
  owner.destroy();
  assert.equal(destroyed, 1);
});
