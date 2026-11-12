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
/** Reused topology belongs only to the existing real native projection issue and its finite mutation observer. */
import assert from 'node:assert/strict';
import test from 'node:test';
import type { ViewUpdate } from '@codemirror/view';
import { admitReorderLayout } from '../../../../../../frontend/sugarprompt/editor/src/reorder-layout-admission.ts';
import {
  borrowNativeReorderLayoutLease,
  currentNativeReorderProjectionTargets,
} from '../../../../../../frontend/sugarprompt/editor/src/reorder-native-projection.ts';
import { currentReorderProjectionLease } from '../../../../../../frontend/sugarprompt/editor/src/reorder-paint-projection.ts';
import { nativeFixture } from './reorder-native-projection-fixture.ts';

/** Closed native identity checks cannot print a painted subtree. */
function identities(actual: readonly Node[] | undefined, expected: readonly object[]): void {
  assert.equal(actual !== undefined, true);
  assert.equal(actual!.length, expected.length);
  assert.equal(
    actual!.every((node, at) => node === expected[at]),
    true,
  );
}

test('real native issue lends complete target and non-target line output without repeating a target child read', () => {
  const t = nativeFixture();
  try {
    const publication = t.publish();
    assert.ok(currentReorderProjectionLease(t.f.view, publication.value!));
    const targets = currentNativeReorderProjectionTargets(t.f.view)!;
    const current = borrowNativeReorderLayoutLease(t.f.view, targets)!;
    assert.equal(typeof current, 'function');
    assert.equal(borrowNativeReorderLayoutLease(t.f.view, [...targets]), null);
    const glyphs = t.f.chips.map((chip) => chip.childNodes[0]!);
    const first = t.f.chips[0]!,
      children = first.childNodes;
    let reads = 0;
    Object.defineProperty(first, 'childNodes', {
      /** The expected output must survive a genuinely unavailable duplicate native read. */
      get() {
        if (++reads > 1) throw new Error('controlled-repeated-native-topology');
        return children;
      },
    });
    const observed = admitReorderLayout(
      targets,
      () => [t.f.line as unknown as HTMLElement],
      current,
    );
    assert.equal(observed !== null, true);
    for (const [at, target] of targets.entries())
      identities(observed!.targets.get(target.node), [target.node, glyphs[at]!]);
    identities(observed!.nodes, [
      t.f.line,
      ...t.f.chips.flatMap((chip, at) => [chip, glyphs[at]!]),
    ]);
    assert.equal(reads, 1);
    assert.equal(t.observers.length, 1);
    assert.equal(current(), true);
  } finally {
    t.owner.destroy();
  }
  assert.equal(
    t.observers.every((observer) => observer.disconnected),
    true,
  );
});

for (const mutation of ['replacement', 'characters', 'overbudget', 'newer-issue'] as const) {
  test(`native ${mutation} at lazy line discovery refuses retained topology without re-admission`, () => {
    const t = nativeFixture();
    try {
      const publication = t.publish();
      assert.ok(currentReorderProjectionLease(t.f.view, publication.value!));
      const targets = currentNativeReorderProjectionTargets(t.f.view)!;
      const current = borrowNativeReorderLayoutLease(t.f.view, targets)!;
      const observed = admitReorderLayout(
        targets,
        () => {
          if (mutation === 'newer-issue') {
            t.owner.update({
              docChanged: false,
              viewportChanged: false,
              geometryChanged: true,
            } as ViewUpdate);
            const next = t.publish();
            assert.ok(currentReorderProjectionLease(t.f.view, next.value!));
          } else {
            const first = t.f.chips[0]!;
            if (mutation === 'replacement') {
              const replacement = t.f.document.createElement('span');
              first.replaceChildren(replacement);
            } else if (mutation === 'characters') {
              Object.defineProperty(first.childNodes[0]!, 'textContent', {
                value: 'x'.repeat(16385),
              });
            } else {
              first.append(
                ...Array.from({ length: 8193 }, () => t.f.document.createElement('span')),
              );
            }
            t.observers[0]!.records.push({} as MutationRecord);
          }
          return [t.f.line as unknown as HTMLElement];
        },
        current,
      );
      assert.equal(observed, null);
      assert.equal(current(), false);
      assert.equal(
        currentNativeReorderProjectionTargets(t.f.view) === null,
        mutation !== 'newer-issue',
      );
      assert.equal(t.observers.length, mutation === 'newer-issue' ? 2 : 1);
    } finally {
      t.owner.destroy();
    }
    assert.equal(
      t.observers.every((observer) => observer.disconnected),
      true,
    );
  });
}

test('mutations during connectivity currentness cannot authorize copied topology', () => {
  const t = nativeFixture();
  try {
    const publication = t.publish();
    assert.ok(currentReorderProjectionLease(t.f.view, publication.value!));
    const targets = currentNativeReorderProjectionTargets(t.f.view)!;
    const current = borrowNativeReorderLayoutLease(t.f.view, targets)!;
    let changed = false;
    Object.defineProperty(t.f.line, 'isConnected', {
      /** Native connectivity can reenter; the lease must drain resulting records too. */
      get() {
        if (!changed) {
          changed = true;
          t.f.chips[0]!.replaceChildren(t.f.text('xx'));
          t.observers[0]!.records.push({} as MutationRecord);
        }
        return true;
      },
    });
    assert.equal(
      admitReorderLayout(targets, () => [t.f.line as unknown as HTMLElement], current),
      null,
    );
    assert.equal(changed, true);
    assert.equal(current(), false);
    assert.equal(t.observers.length, 1);
  } finally {
    t.owner.destroy();
  }
});
