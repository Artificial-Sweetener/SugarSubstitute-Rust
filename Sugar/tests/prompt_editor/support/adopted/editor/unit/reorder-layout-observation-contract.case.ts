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
/** Independent existing-owner characterization binds the two admission roles before observation fusion. */
import assert from 'node:assert/strict';
import test from 'node:test';
import {
  admitReorderLayout,
  admitReorderLineLayout,
  admitReorderTargetLayout,
} from '../../../../../../frontend/sugarprompt/editor/src/reorder-layout-admission.ts';
import type { MountedReorderTarget } from '../../../../../../frontend/sugarprompt/editor/src/reorder-mounted-target.ts';
import { motionPaintFixture } from './reorder-motion-paint-fixture.ts';

/** The established native transport owns actual child and parent identities. */
type Paint = ReturnType<typeof motionPaintFixture>;
/** Adapt only the fixture's typed external DOM boundary. */
function element(node: ReturnType<Paint['document']['createElement']>): HTMLElement {
  return node as unknown as HTMLElement;
}
/** Native semantic targets carry explicit independent role metadata. */
function target(
  node: HTMLElement,
  index: number,
  kind: MountedReorderTarget['kind'] = 'chip',
): MountedReorderTarget {
  return { node, index, kind, leases: [], origins: [] };
}
/** Closed identity assertions cannot print a painted subtree. */
function identities(actual: readonly Node[] | undefined | null, expected: readonly object[]): void {
  assert.equal(actual !== null && actual !== undefined, true);
  assert.equal(actual!.length, expected.length);
  assert.equal(
    actual!.every((node, index) => node === expected[index]),
    true,
  );
}

for (const first of ['targets', 'lines'] as const) {
  test(`independent ${first}-first roles preserve non-targets, reverse root order and carrier multiplicity`, () => {
    const f = motionPaintFixture(),
      line = f.document.createElement('span'),
      outside = f.document.createElement('span'),
      carrier = f.text('x'),
      chip = f.document.createElement('span'),
      glyph = f.text('y'),
      gap = f.document.createElement('span'),
      partition = f.document.createElement('span');
    chip.append(glyph);
    line.append(carrier, chip, gap, partition);
    const targets = [
      target(element(chip), 0),
      target(element(gap), 1, 'gap'),
      target(element(partition), 2, 'partition'),
    ];
    const readTargets = () => {
      const admitted = admitReorderTargetLayout(targets);
      assert.equal(admitted?.size, 3);
      identities(admitted!.get(element(chip)), [chip, glyph]);
      identities(admitted!.get(element(gap)), [carrier, gap]);
      identities(admitted!.get(element(partition)), [carrier, partition]);
    };
    const readLines = () =>
      identities(admitReorderLineLayout([element(outside), element(line)]), [
        line,
        carrier,
        chip,
        glyph,
        gap,
        partition,
        outside,
      ]);
    if (first === 'targets') {
      readTargets();
      readLines();
    } else {
      readLines();
      readTargets();
    }
  });
}

test('separate role character budgets charge a shared carrier once in the line and twice among targets', () => {
  const f = motionPaintFixture(),
    line = f.document.createElement('span'),
    carrier = f.text('x'.repeat(8192)),
    first = f.document.createElement('span'),
    second = f.document.createElement('span');
  line.append(carrier, first, second);
  const targets = [target(element(first), 0, 'gap'), target(element(second), 1, 'partition')];
  identities(admitReorderLineLayout([element(line)]), [line, carrier, first, second]);
  const admitted = admitReorderTargetLayout(targets)!;
  identities(admitted.get(element(first)), [carrier, first]);
  identities(admitted.get(element(second)), [carrier, second]);
  Object.defineProperty(carrier, 'length', { value: 8193 });
  assert.equal(admitReorderLineLayout([element(line)]) !== null, true);
  assert.equal(admitReorderTargetLayout(targets) === null, true);
});

test('line and target node frontiers independently include 8192 occurrences and refuse pending siblings', () => {
  const f = motionPaintFixture(),
    root = f.document.createElement('span'),
    pending = f.document.createElement('span'),
    children = Array.from({ length: 8191 }, () => f.document.createElement('span'));
  root.append(...children);
  identities(admitReorderLineLayout([element(root)]), [root, ...children]);
  identities(admitReorderTargetLayout([target(element(root), 0)])!.get(element(root)), [
    root,
    ...children,
  ]);
  let forbidden = 0;
  Object.defineProperty(children[0]!, 'childNodes', {
    /** Refusal must precede any descendant collection read. */
    get() {
      forbidden++;
      // eslint-disable-next-line @typescript-eslint/only-throw-error -- Inject raw host failures to prove original cause retention and cleanup.
      throw undefined;
    },
  });
  assert.equal(admitReorderLineLayout([element(pending), element(root)]) === null, true);
  assert.equal(
    admitReorderTargetLayout([target(element(pending), 0), target(element(root), 1)]) === null,
    true,
  );
  assert.equal(forbidden, 0);
});

for (const raw of [null, undefined, Object.freeze({ code: 'controlled-first-read' })]) {
  test('the first performed native failure remains the raw cause', () => {
    const f = motionPaintFixture(),
      root = f.document.createElement('span');
    let reads = 0,
      thrown = false,
      actual: unknown;
    Object.defineProperty(root, 'childNodes', {
      /** Preserve the exact first native failure, including null and undefined. */
      get() {
        reads++;
        // eslint-disable-next-line @typescript-eslint/only-throw-error -- Inject raw host failures to prove original cause retention and cleanup.
        throw raw;
      },
    });
    try {
      admitReorderLineLayout([element(root)]);
    } catch (cause) {
      thrown = true;
      actual = cause;
    }
    assert.equal(thrown, true);
    assert.equal(actual === raw, true);
    assert.equal(reads, 1);
  });
}

test('one guarded observation reuses actual target topology while retaining complete line output and carrier occurrences', () => {
  const f = motionPaintFixture(),
    line = f.document.createElement('span'),
    outside = f.document.createElement('span'),
    carrier = f.text('x'),
    chip = f.document.createElement('span'),
    glyph = f.text('y'),
    gap = f.document.createElement('span'),
    partition = f.document.createElement('span');
  chip.append(glyph);
  line.append(carrier, chip, gap, partition);
  const targets = [
    target(element(chip), 0),
    target(element(gap), 1, 'gap'),
    target(element(partition), 2, 'partition'),
  ];
  let chipReads = 0;
  const children = chip.childNodes;
  Object.defineProperty(chip, 'childNodes', {
    /** A second physical observation cannot produce these independently required outputs. */
    get() {
      if (++chipReads > 1) throw new Error('controlled-duplicate-topology-read');
      return children;
    },
  });
  const admitted = admitReorderLayout(
    targets,
    () => [element(outside), element(line)],
    () => true,
  );
  assert.equal(admitted !== null, true);
  identities(admitted!.targets.get(element(chip)), [chip, glyph]);
  identities(admitted!.targets.get(element(gap)), [carrier, gap]);
  identities(admitted!.targets.get(element(partition)), [carrier, partition]);
  identities(admitted!.nodes, [line, carrier, chip, glyph, gap, partition, outside]);
  assert.equal(chipReads, 1);
});

for (const length of [8192, 8193]) {
  test('joint observation retains independent line and repeated-carrier target character budgets', () => {
    const f = motionPaintFixture(),
      line = f.document.createElement('span'),
      carrier = f.text('x'.repeat(length)),
      first = f.document.createElement('span'),
      second = f.document.createElement('span');
    line.append(carrier, first, second);
    let discoveries = 0;
    const admitted = admitReorderLayout(
      [target(element(first), 0, 'gap'), target(element(second), 1, 'partition')],
      () => {
        discoveries++;
        return [element(line)];
      },
      () => true,
    );
    assert.equal(admitted !== null, length === 8192);
    assert.equal(discoveries, length === 8192 ? 1 : 0);
    if (admitted) identities(admitted.nodes, [line, carrier, first, second]);
  });
}

test('a complete line still refuses its own pending frontier after a small target was admitted', () => {
  const f = motionPaintFixture(),
    chip = f.document.createElement('span'),
    line = f.document.createElement('span'),
    sibling = f.document.createElement('span');
  line.append(...Array.from({ length: 8191 }, () => f.document.createElement('span')));
  assert.equal(
    admitReorderLayout(
      [target(element(chip), 0)],
      () => [element(sibling), element(line)],
      () => true,
    ),
    null,
  );
});

for (const raw of [null, undefined, Object.freeze({ code: 'controlled-first-joint-read' })]) {
  test('joint observation preserves original first target failure before any freshness or line callback', () => {
    const f = motionPaintFixture(),
      chip = f.document.createElement('span');
    Object.defineProperty(chip, 'childNodes', {
      get() {
        // eslint-disable-next-line @typescript-eslint/only-throw-error -- Inject raw host failures to prove original cause retention and cleanup.
        throw raw;
      },
    });
    let callbacks = 0,
      caught = false,
      actual: unknown;
    try {
      admitReorderLayout(
        [target(element(chip), 0)],
        () => {
          callbacks++;
          return [];
        },
        () => {
          callbacks++;
          return true;
        },
      );
    } catch (cause) {
      caught = true;
      actual = cause;
    }
    assert.equal(caught, true);
    assert.equal(actual === raw, true);
    assert.equal(callbacks, 0);
  });
}
