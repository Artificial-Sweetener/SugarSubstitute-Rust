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
/** Copied Stage topology is shared only within one observed native READ; browser proof owns real geometry/performance. */
import assert from 'node:assert/strict';
import test from 'node:test';
import { observeReorderSlotLayout } from '../../../../../../frontend/sugarprompt/editor/src/reorder-slot-layout-read.ts';
import type { MountedReorderTarget } from '../../../../../../frontend/sugarprompt/editor/src/reorder-mounted-target.ts';
import type { ReorderLineLayoutRead } from '../../../../../../frontend/sugarprompt/editor/src/reorder-layout-admission.ts';
import { motionPaintFixture } from './reorder-motion-paint-fixture.ts';

/** Literal target operands carry no derived expected topology or admission logic. */
function target(
  node: HTMLElement,
  index: number,
  kind: MountedReorderTarget['kind'] = 'chip',
): MountedReorderTarget {
  return { node, index, kind, leases: [], origins: [] };
}
/** Compare actual output references against independently authored literal node order. */
function identities(actual: readonly Node[] | undefined, expected: readonly Node[]): void {
  assert.ok(actual);
  assert.equal(actual.length, expected.length);
  for (const [index, node] of expected.entries()) assert.equal(actual[index] === node, true);
}
/** Own the body and every cleanup independently, preserving raw primary failures including null/undefined. */
function complete(action: (f: ReturnType<typeof motionPaintFixture>) => void): void {
  const f = motionPaintFixture(),
    causes: unknown[] = [];
  try {
    action(f);
  } catch (cause) {
    causes.push(cause);
  }
  for (const release of [() => assert.equal(f.nativeMutations.active, 0), () => f.destroyLayer()])
    try {
      release();
    } catch (cause) {
      causes.push(cause);
    }
  if (causes.length === 1) throw causes[0];
  if (causes.length) throw new AggregateError(causes, 'slot-layout-proof-retirement');
}

test('one shell-first request reuses actual children/text operands and exact chip/carrier output, then revokes its snapshot', () => {
  complete((f) => {
    const document = f.view.dom.ownerDocument;
    const shell = document.createElement('span'),
      carrier = document.createTextNode('p'),
      chip = document.createElement('span'),
      glyph = document.createTextNode('g'),
      gap = document.createElement('span'),
      partition = document.createElement('span');
    chip.append(glyph);
    shell.append(carrier, chip, gap, partition);
    const nodes = [shell, carrier, chip, glyph, gap, partition];
    const counts = nodes.map((node) => {
      const children = node.childNodes;
      let count = 0;
      Object.defineProperty(node, 'childNodes', {
        configurable: true,
        get() {
          if (++count > 1) throw new Error('duplicate-copied-topology-read');
          return children;
        },
      });
      return () => count;
    });
    const lengths = [carrier, glyph].map((node) => {
      const length = node.length;
      let count = 0;
      Object.defineProperty(node, 'length', {
        configurable: true,
        get() {
          if (++count > 1) throw new Error('duplicate-copied-text-length-read');
          return length;
        },
      });
      return () => count;
    });
    const borrowed: { read: ReorderLineLayoutRead | null } = { read: null };
    let callbacks = 0;
    const result = observeReorderSlotLayout(
      shell,
      () => true,
      (read, authority) => {
        callbacks++;
        borrowed.read = read;
        identities(read.nodes, nodes);
        const admitted = read.targets([
          target(chip, 0),
          target(gap, 1, 'gap'),
          target(partition, 2, 'partition'),
        ]);
        assert.ok(admitted);
        identities(admitted.get(chip), [chip, glyph]);
        identities(admitted.get(gap), [carrier, gap]);
        identities(admitted.get(partition), [carrier, partition]);
        assert.equal(authority.guard(authority.available), true);
        return admitted;
      },
    );
    assert.ok(result);
    assert.equal(callbacks, 1);
    assert.deepEqual(
      counts.map((read) => read()),
      [1, 1, 1, 1, 1, 1],
    );
    assert.deepEqual(
      lengths.map((read) => read()),
      [1, 1],
    );
    assert.ok(borrowed.read);
    // The snapshot cannot retain authority after its native observer has disconnected.
    assert.equal(borrowed.read.targets([target(chip, 0)]), null);
  });
});

for (const characters of [8192, 8193])
  test(`repeated carrier target character budget remains independent at ${characters}`, () => {
    complete((f) => {
      const document = f.view.dom.ownerDocument,
        shell = document.createElement('span'),
        carrier = document.createTextNode('x'.repeat(characters)),
        gap = document.createElement('span'),
        partition = document.createElement('span');
      shell.append(carrier, gap, partition);
      let calls = 0;
      const result = observeReorderSlotLayout(
        shell,
        () => true,
        (read) => {
          calls++;
          identities(read.nodes, [shell, carrier, gap, partition]);
          return read.targets([target(gap, 0, 'gap'), target(partition, 1, 'partition')]);
        },
      );
      assert.equal(calls, 1);
      assert.equal(result !== null, characters === 8192);
      if (result) {
        identities(result.get(gap), [carrier, gap]);
        identities(result.get(partition), [carrier, partition]);
      }
    });
  });

for (const children of [4094, 4095])
  test(`repeated carrier target node budget remains independent with ${children} descendants`, () => {
    complete((f) => {
      const document = f.view.dom.ownerDocument,
        shell = document.createElement('span'),
        carrier = document.createElement('span'),
        gap = document.createElement('span'),
        partition = document.createElement('span');
      for (let index = 0; index < children; index++) carrier.append(document.createElement('span'));
      shell.append(carrier, gap, partition);
      const result = observeReorderSlotLayout(
        shell,
        () => true,
        (read) => read.targets([target(gap, 0, 'gap'), target(partition, 1, 'partition')]),
      );
      assert.equal(result !== null, children === 4094);
      if (result) assert.equal(result.get(gap)!.length + result.get(partition)!.length, 8192);
    });
  });

for (const mutation of ['shell-child', 'shell-text', 'target-parent', 'geometry-text'] as const)
  test(`same-request ${mutation} mutation refuses copied output and disconnects`, () => {
    complete((f) => {
      const document = f.view.dom.ownerDocument,
        shell = document.createElement('span'),
        chip = document.createElement('span'),
        text = document.createTextNode('a');
      chip.append(text);
      shell.append(chip);
      let calls = 0,
        reads = 0;
      if (mutation === 'shell-child') {
        const children = chip.childNodes;
        Object.defineProperty(chip, 'childNodes', {
          get() {
            if (++reads === 1) chip.append(document.createElement('i'));
            return children;
          },
        });
      } else if (mutation === 'shell-text') {
        Object.defineProperty(text, 'length', {
          get() {
            if (++reads === 1) text.data = 'b';
            return 1;
          },
        });
      } else if (mutation === 'target-parent') {
        Object.defineProperty(chip, 'parentNode', {
          get() {
            shell.setAttribute('data-foreign', '1');
            return shell;
          },
        });
      } else
        chip.getClientRects = () => {
          text.data = 'b';
          return [] as unknown as DOMRectList;
        };
      const result = observeReorderSlotLayout(
        shell,
        () => true,
        (read, authority) => {
          calls++;
          const targets = read.targets([
            target(chip, 0, mutation === 'target-parent' ? 'gap' : 'chip'),
          ]);
          if (!targets) return null;
          if (mutation === 'geometry-text') chip.getClientRects();
          return authority.guard(authority.available) ? targets : null;
        },
      );
      assert.equal(result, null);
      assert.equal(calls, mutation === 'shell-child' || mutation === 'shell-text' ? 0 : 1);
    });
  });

/** A changed native observer binding has one exact reversible lifetime, preserving every raw body/restore cause. */
function observerBoundary(
  window: Window,
  value: typeof MutationObserver | undefined,
  action: () => void,
): void {
  const descriptor = Object.getOwnPropertyDescriptor(window, 'MutationObserver'),
    causes: unknown[] = [];
  try {
    if (value)
      Object.defineProperty(window, 'MutationObserver', {
        configurable: true,
        value,
      });
    else Reflect.deleteProperty(window, 'MutationObserver');
    action();
  } catch (cause) {
    causes.push(cause);
  }
  try {
    if (descriptor) Object.defineProperty(window, 'MutationObserver', descriptor);
    else Reflect.deleteProperty(window, 'MutationObserver');
  } catch (cause) {
    causes.push(cause);
  }
  if (causes.length === 1) throw causes[0];
  if (causes.length) throw new AggregateError(causes, 'slot-layout-native-binding-retirement');
}

test('observer absence is explicit unavailability before any action or shell discovery', () => {
  complete((f) => {
    const window = f.view.dom.ownerDocument.defaultView!,
      shell = f.view.dom.ownerDocument.createElement('span');
    let calls = 0;
    observerBoundary(window, undefined, () => {
      assert.equal(
        observeReorderSlotLayout(
          shell,
          () => true,
          () => {
            calls++;
            return true;
          },
        ),
        null,
      );
      assert.equal(calls, 0);
    });
  });
});

for (const raw of [null, undefined, Object.freeze({ code: 'slot-read-primary' })])
  test('raw action cause precedes independent native disconnect failure', () => {
    complete((f) => {
      const window = f.view.dom.ownerDocument.defaultView!,
        Native = window.MutationObserver;
      const cleanup = Object.freeze({ code: 'slot-read-cleanup' });
      let disconnected = false,
        finalChecks = 0;
      class Observer extends Native {
        /** The captured native observer retires before its controlled external failure. */
        override disconnect(): void {
          super.disconnect();
          disconnected = true;
          // eslint-disable-next-line @typescript-eslint/only-throw-error -- Inject raw host failures to prove original cause retention and cleanup.
          throw cleanup;
        }
      }
      observerBoundary(window, Observer, () => {
        let caught = false,
          cause: unknown;
        try {
          observeReorderSlotLayout(
            f.view.dom.ownerDocument.createElement('span'),
            () => {
              if (disconnected) {
                finalChecks++;
                throw new Error('unreachable-final-owner-check');
              }
              return true;
            },
            () => {
              // eslint-disable-next-line @typescript-eslint/only-throw-error -- Inject raw host failures to prove original cause retention and cleanup.
              throw raw;
            },
          );
        } catch (error) {
          caught = true;
          cause = error;
        }
        assert.equal(caught, true);
        assert.ok(cause instanceof AggregateError);
        assert.equal(cause.errors[0] === raw, true);
        assert.equal(cause.errors[1] === cleanup, true);
        assert.equal(cause.errors.length, 2);
        assert.equal(finalChecks, 0);
      });
    });
  });

for (const characters of [16384, 16385])
  test(`whole shell retains its independent inclusive character limit at ${characters}`, () => {
    complete((f) => {
      const document = f.view.dom.ownerDocument,
        shell = document.createElement('span'),
        text = document.createTextNode('x'.repeat(characters));
      shell.append(text);
      let calls = 0;
      const result = observeReorderSlotLayout(
        shell,
        () => true,
        (read) => {
          calls++;
          identities(read.nodes, [shell, text]);
          return true;
        },
      );
      assert.equal(result === true, characters === 16384);
      assert.equal(calls, characters === 16384 ? 1 : 0);
    });
  });

for (const children of [8191, 8192])
  test(`whole shell retains its independent inclusive node limit with ${children} children`, () => {
    complete((f) => {
      const document = f.view.dom.ownerDocument,
        shell = document.createElement('span'),
        nodes: HTMLElement[] = [];
      for (let index = 0; index < children; index++) {
        const node = document.createElement('span');
        nodes.push(node);
        shell.append(node);
      }
      let calls = 0;
      const result = observeReorderSlotLayout(
        shell,
        () => true,
        (read) => {
          calls++;
          identities(read.nodes, [shell, ...nodes]);
          return true;
        },
      );
      assert.equal(result === true, children === 8191);
      assert.equal(calls, children === 8191 ? 1 : 0);
    });
  });

for (const raw of [null, undefined, Object.freeze({ code: 'final-slot-owner-check' })])
  test('post-cleanup owner failure stays raw after successful action and disconnect', () => {
    complete((f) => {
      const window = f.view.dom.ownerDocument.defaultView!,
        Native = window.MutationObserver;
      let disconnected = false,
        calls = 0,
        caught = false,
        cause: unknown;
      class Observer extends Native {
        /** Final owner authority is consulted only after this native subscription has actually retired. */
        override disconnect(): void {
          super.disconnect();
          disconnected = true;
        }
      }
      observerBoundary(window, Observer, () => {
        try {
          observeReorderSlotLayout(
            f.view.dom.ownerDocument.createElement('span'),
            () => {
              // eslint-disable-next-line @typescript-eslint/only-throw-error -- Inject raw host failures to prove original cause retention and cleanup.
              if (disconnected) throw raw;
              return true;
            },
            () => {
              calls++;
              return true;
            },
          );
        } catch (error) {
          caught = true;
          cause = error;
        }
        assert.equal(caught, true);
        assert.equal(cause === raw, true);
        assert.equal(disconnected, true);
        assert.equal(calls, 1);
      });
    });
  });
