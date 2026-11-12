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
/** Candidate coverage excludes only provably ineligible whole IDs and retains complete native root operands. */
import assert from 'node:assert/strict';
import test from 'node:test';
import { ReorderMountedLayout } from '../../../../../../frontend/sugarprompt/editor/src/reorder-mounted-layout.ts';
import { observeReorderSlotLayout } from '../../../../../../frontend/sugarprompt/editor/src/reorder-slot-layout-read.ts';
import type {
  MountedReorderTarget,
  ReorderMountedObservation,
} from '../../../../../../frontend/sugarprompt/editor/src/reorder-mounted-target.ts';
import { motionPresentationFixture } from './reorder-motion-presentation-fixture.ts';
import { candidateBox, candidateRects } from './reorder-candidate-read-fixture.ts';

/** Every native root/rich rectangle is independently authored; the actual Slot observer and Layout own all admission. */
function usingRoots(
  action: (f: {
    readonly owner: ReorderMountedLayout;
    readonly shell: HTMLElement;
    readonly targets: MountedReorderTarget[];
    readonly roots: HTMLElement[];
    readonly rich: HTMLElement[];
  }) => void,
): void {
  const f = motionPresentationFixture(),
    owner = new ReorderMountedLayout(f.view),
    shell = f.view.dom.ownerDocument.createElement('span'),
    causes: unknown[] = [];
  try {
    f.root.closest<HTMLElement>('.cm-line')!.append(shell);
    const roots: HTMLElement[] = [],
      rich: HTMLElement[] = [],
      targets: MountedReorderTarget[] = [];
    for (const [index, left, width] of [
      [1, 10, 10],
      [2, 30, 10],
      [2, 95, 10],
      [3, 50, 0],
    ]) {
      const root = shell.ownerDocument.createElement('span'),
        child = shell.ownerDocument.createElement('span');
      root.append(child);
      shell.append(root);
      root.getBoundingClientRect = () => candidateBox(left!, width!);
      root.getClientRects = () => candidateRects([candidateBox(left!, width!)]);
      child.getClientRects = () => candidateRects([candidateBox(left!, width === 0 ? 5 : width!)]);
      roots.push(root);
      rich.push(child);
      targets.push({
        node: root,
        index: index!,
        kind: 'chip',
        leases: [],
        origins: [],
      });
    }
    action({ owner, shell, targets, roots, rich });
  } catch (cause) {
    causes.push(cause);
  }
  for (const release of [() => owner.destroy(), () => shell.remove(), () => f.destroy()])
    try {
      release();
    } catch (cause) {
      causes.push(cause);
    }
  if (causes.length === 1) throw causes[0];
  if (causes.length) throw new AggregateError(causes, 'candidate-root-proof-retirement');
}

test('one outside positive root omits every root of its ID while zero-area roots retain actual rich output and same-READ completion reuses all native samples', () => {
  usingRoots((f) => {
    const coverage = {
        kind: 'carrier-candidates' as const,
        viewport: { left: 0, right: 100, top: 0, bottom: 100 },
      },
      counts = f.roots.map(() => 0),
      richCounts = f.rich.map(() => 0);
    for (const [index, root] of f.roots.entries()) {
      const native = root.getClientRects;
      root.getClientRects = () => {
        if (++counts[index]! > 1) assert.fail('same-read-root-rediscovered');
        return native.call(root);
      };
      const child = f.rich[index]!,
        physical = child.getClientRects;
      child.getClientRects = () => {
        richCounts[index]!++;
        return physical.call(child);
      };
    }
    const borrowed: { observed: ReorderMountedObservation | null } = {
      observed: null,
    };
    const full = observeReorderSlotLayout(
      f.shell,
      () => true,
      (topology, authority) => {
        const read = f.owner.observe(f.targets, authority, topology.targets, coverage);
        assert.ok(read?.coverage === 'carrier-candidates');
        borrowed.observed = read;
        assert.deepEqual([...read.omitted], [2]);
        assert.deepEqual([...read.footprints.keys()], [f.roots[0], f.roots[3]]);
        assert.deepEqual(counts, [1, 1, 1, 1]);
        assert.deepEqual(richCounts, [1, 0, 0, 1]);
        const complete = read.complete();
        assert.ok(complete?.coverage === 'complete');
        assert.equal(complete.layout, read.layout);
        assert.deepEqual([...complete.footprints.keys()], f.roots);
        assert.deepEqual(counts, [1, 1, 1, 1]);
        assert.deepEqual(richCounts, [1, 1, 1, 1]);
        return complete;
      },
    );
    assert.ok(full);
    assert.ok(borrowed.observed?.coverage === 'carrier-candidates');
    assert.equal(borrowed.observed.complete(), null);
    assert.deepEqual(counts, [1, 1, 1, 1]);
    assert.deepEqual(richCounts, [1, 1, 1, 1]);
  });
});

test('candidate mode preserves4097 native root-list rejection before any item or later target read', () => {
  usingRoots((f) => {
    let items = 0,
      later = 0;
    f.roots[0]!.getClientRects = () =>
      new Proxy(candidateRects([]), {
        get(_target, key) {
          if (key === 'length') return 4097;
          items++;
          throw new Error('overbudget-root-item-read');
        },
      });
    f.roots[1]!.getClientRects = () => {
      later++;
      assert.fail('overbudget-read-later-root');
    };
    const read = observeReorderSlotLayout(
      f.shell,
      () => true,
      (topology, authority) =>
        f.owner.observe(f.targets, authority, topology.targets, {
          kind: 'carrier-candidates',
          viewport: { left: 0, right: 1, top: 0, bottom: 1 },
        }),
    );
    assert.equal(read, null);
    assert.equal(items, 0);
    assert.equal(later, 0);
  });
});

test('candidate root capture rejects synchronous source retirement before consuming a returned native operand', () => {
  usingRoots((f) => {
    let current = true,
      consumed = 0;
    f.roots[0]!.getClientRects = () => {
      current = false;
      return new Proxy(candidateRects([]), {
        get() {
          consumed++;
          throw new Error('retired-candidate-operand-consumed');
        },
      });
    };
    const read = observeReorderSlotLayout(
      f.shell,
      () => current,
      (topology, authority) =>
        f.owner.observe(f.targets, authority, topology.targets, {
          kind: 'carrier-candidates',
          viewport: { left: 0, right: 100, top: 0, bottom: 100 },
        }),
    );
    assert.equal(read, null);
    assert.equal(consumed, 0);
  });
});

test('complete coverage charges every repeated target occurrence before the original aggregate fragment frontier', () => {
  usingRoots((f) => {
    let roots = 0,
      rich = 0;
    f.roots[0]!.getClientRects = () => {
      roots++;
      return candidateRects(Array.from({ length: 16 }, () => candidateBox(10, 10)));
    };
    f.rich[0]!.getClientRects = () => {
      rich++;
      return candidateRects([candidateBox(10, 10)]);
    };
    const read = observeReorderSlotLayout(
      f.shell,
      () => true,
      (topology, authority) =>
        f.owner.observe(
          Array.from({ length: 257 }, () => f.targets[0]!),
          authority,
          topology.targets,
          { kind: 'complete' },
        ),
    );
    assert.equal(read, null);
    // Each occurrence contributes16 root fragments plus one descendant; occurrence241 crosses4096.
    assert.equal(roots, 241);
    assert.equal(rich, 241);
  });
});

test('an empty native root fragment list cannot exclude its positive rich descendants', () => {
  usingRoots((f) => {
    f.roots[2]!.getClientRects = () => candidateRects([]);
    const read = observeReorderSlotLayout(
      f.shell,
      () => true,
      (topology, authority) => {
        const observed = f.owner.observe(f.targets, authority, topology.targets, {
          kind: 'carrier-candidates',
          viewport: { left: 0, right: 100, top: 0, bottom: 100 },
        });
        assert.ok(observed?.coverage === 'carrier-candidates');
        assert.deepEqual([...observed.omitted], []);
        assert.deepEqual(observed.footprints.get(f.roots[2]!), [
          { kind: 'chip', index: 2, left: 95, right: 105, top: 20, bottom: 40 },
        ]);
        return observed;
      },
    );
    assert.ok(read);
  });
});

for (const invalidation of ['source', 'topology'] as const)
  test(`same-READ ${invalidation} invalidation revokes omitted-rich completion before its next native read`, () => {
    usingRoots((f) => {
      let current = true,
        missing = 0;
      for (const index of [1, 2])
        f.rich[index]!.getClientRects = () => {
          missing++;
          assert.fail('revoked-missing-rich-read');
        };
      const read = observeReorderSlotLayout(
        f.shell,
        () => current,
        (topology, authority) => {
          const observed = f.owner.observe(f.targets, authority, topology.targets, {
            kind: 'carrier-candidates',
            viewport: { left: 0, right: 100, top: 0, bottom: 100 },
          });
          assert.ok(observed?.coverage === 'carrier-candidates');
          if (invalidation === 'source') current = false;
          else f.shell.append(f.shell.ownerDocument.createElement('span'));
          assert.equal(observed.complete(), null);
          return observed;
        },
      );
      assert.equal(read, null);
      assert.equal(missing, 0);
    });
  });
