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
/** Delegated admission cannot revive a retired finite native carrier owner or authorize later native work. */
import { admitReorderTargetLayout } from '../../../../../../frontend/sugarprompt/editor/src/reorder-layout-admission.ts';
import assert from 'node:assert/strict';
import test from 'node:test';
import { ReorderMotionPeerCarriers } from '../../../../../../frontend/sugarprompt/editor/src/reorder-motion-peer-carriers.ts';
import { usingAtomicPaintFixture, atomicScenePaintSource } from './reorder-atomic-paint-fixture.ts';

for (const phase of [
  'read-prepared',
  'create',
  'read-instantiated',
  'validate',
  'reveal',
  'frame',
] as const)
  test(`${phase} admission self-retirement stops subsequent native allocation, geometry and paint`, () =>
    usingAtomicPaintFixture((f) => {
      const chips = [atomicScenePaintSource(f, 0)],
        root = f.roots[0]!;
      const owner = ReorderMotionPeerCarriers.prepare(
        f.view,
        f.f.line as unknown as HTMLElement,
        chips,
        1,
        1,
        () => true,
      );
      assert.ok(owner);
      try {
        if (phase !== 'read-prepared' && phase !== 'create')
          assert.equal(
            owner.instantiate(() => true),
            true,
          );
        if (phase === 'reveal' || phase === 'frame')
          assert.equal(
            owner.read(chips, () => true, admitReorderTargetLayout),
            'ready',
          );
        if (phase === 'frame')
          assert.equal(
            owner.reveal(() => true),
            true,
          );
        const carrier = f.f.line.querySelector('.sp-reorder-motion-peer'),
          copy = carrier?.firstElementChild;
        let allocations = 0,
          geometry = 0,
          paint = 0;
        f.f.document.createElement = () => {
          allocations++;
          throw new Error('retired owner allocated native output');
        };
        if (copy)
          copy.getClientRects = () => {
            geometry++;
            throw new Error('retired owner read native geometry');
          };
        if (carrier)
          Object.defineProperty(carrier.style, 'transform', {
            configurable: true,
            set: () => {
              paint++;
              throw new Error('retired owner wrote native frame');
            },
          });
        const current = () => {
          owner.destroy();
          return true;
        };
        if (phase === 'create') assert.equal(owner.instantiate(current), false);
        else if (phase === 'validate')
          assert.equal(owner.validate(current, admitReorderTargetLayout), false);
        else if (phase === 'reveal') assert.equal(owner.reveal(current), false);
        else if (phase === 'frame') assert.equal(owner.paint(0, 30, 25, current), true);
        else assert.equal(owner.read(chips, current, admitReorderTargetLayout), 'unavailable');
        assert.deepEqual([allocations, geometry, paint], [0, 0, 0]);
        assert.equal(
          owner.read(chips, () => true, admitReorderTargetLayout),
          'unavailable',
        );
        assert.equal(f.f.line.querySelectorAll('.sp-reorder-motion-peer').length, 0);
        assert.equal(root.getAttribute('data-reorder-motion-flow'), null);
        assert.equal(root.style.getPropertyValue('opacity'), '');
      } finally {
        owner.destroy();
      }
    }));

test('nested footprint admission rejects callback self-retirement before reading the returned native list', () =>
  usingAtomicPaintFixture((f) => {
    const chips = [atomicScenePaintSource(f, 0)];
    const owner = ReorderMotionPeerCarriers.prepare(
      f.view,
      f.f.line as unknown as HTMLElement,
      chips,
      1,
      1,
      () => true,
    );
    assert.ok(owner);
    try {
      assert.equal(
        owner.instantiate(() => true),
        true,
      );
      const copy = f.f.line.querySelector('.sp-reorder-motion-peer')!.firstElementChild!;
      let delegated = false,
        rects = 0,
        lengths = 0;
      const native = new Proxy([] as DOMRect[], {
        get(target, key, receiver) {
          if (key === 'length') {
            lengths++;
            throw new Error('retired footprint read native length');
          }
          return Reflect.get(target, key, receiver);
        },
      });
      copy.getClientRects = () => {
        rects++;
        delegated = true;
        return native;
      };
      const current = () => {
        if (delegated) owner.destroy();
        return true;
      };
      assert.equal(owner.validate(current, admitReorderTargetLayout), false);
      assert.deepEqual([rects, lengths], [1, 0]);
      assert.equal(
        owner.read(chips, () => true, admitReorderTargetLayout),
        'unavailable',
      );
      assert.equal(f.f.line.querySelectorAll('.sp-reorder-motion-peer').length, 0);
    } finally {
      owner.destroy();
    }
  }));

for (const phase of ['read', 'create', 'validate', 'reveal', 'frame'] as const)
  test(`${phase} actual invoked admission preserves its raw thrown cause`, () =>
    usingAtomicPaintFixture((f) => {
      const chips = [atomicScenePaintSource(f, 0)],
        failure = { code: 'literal-admission-callback' };
      const owner = ReorderMotionPeerCarriers.prepare(
        f.view,
        f.f.line as unknown as HTMLElement,
        chips,
        1,
        1,
        () => true,
      );
      assert.ok(owner);
      try {
        if (phase === 'validate' || phase === 'reveal' || phase === 'frame')
          assert.equal(
            owner.instantiate(() => true),
            true,
          );
        if (phase === 'reveal' || phase === 'frame')
          assert.equal(
            owner.read(chips, () => true, admitReorderTargetLayout),
            'ready',
          );
        if (phase === 'frame')
          assert.equal(
            owner.reveal(() => true),
            true,
          );
        const current = () => {
          // eslint-disable-next-line @typescript-eslint/only-throw-error -- Inject raw host failures to prove original cause retention and cleanup.
          throw failure;
        };
        assert.throws(
          () => {
            if (phase === 'read') owner.read(chips, current, admitReorderTargetLayout);
            else if (phase === 'create') owner.instantiate(current);
            else if (phase === 'validate') owner.validate(current, admitReorderTargetLayout);
            else if (phase === 'reveal') owner.reveal(current);
            else owner.paint(0, 30, 25, current);
          },
          (cause) => cause === failure,
        );
      } finally {
        owner.destroy();
      }
    }));
