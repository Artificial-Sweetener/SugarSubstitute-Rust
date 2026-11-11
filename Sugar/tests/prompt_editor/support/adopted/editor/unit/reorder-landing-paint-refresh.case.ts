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
/** Exact landing refresh reuses the actual SVG stroke while preserving publication and raw retirement. */
import assert from 'node:assert/strict';
import test from 'node:test';
import { ReorderLandingPaint } from '../../../../../../frontend/sugarprompt/editor/src/reorder-landing-paint.ts';
import { motionPaintFixture } from './reorder-motion-paint-fixture.ts';
const viewport = { left: -1000, right: 1000, top: -1000, bottom: 1000 };
/** The existing literal transport supplies the actual allocated path and native declaration boundary. */
type NativeLandingNode = ReturnType<typeof motionPaintFixture>['chips'][number];
/** One finite acquired surface keeps assertion/native primary causes ahead of independent retirement. */
function withLandingSurface(
  action: (value: {
    readonly landing: ReorderLandingPaint;
    readonly node: NativeLandingNode;
    readonly path: NativeLandingNode;
    readonly publication: ReturnType<ReorderLandingPaint['prepare']>;
    readonly bubbles: Parameters<ReorderLandingPaint['prepare']>[0];
    readonly body: NativeLandingNode;
  }) => void,
): void {
  const f = motionPaintFixture(),
    landing = new ReorderLandingPaint(f.document as unknown as Document),
    causes: unknown[] = [];
  try {
    const bubbles = [{ left: 6, right: 34, top: 18, bottom: 42 }],
      publication = landing.prepare(bubbles, 'black', false, viewport);
    assert.equal(
      publication.commit(() => true),
      true,
    );
    const body = f.document.body!,
      node = body.querySelector('.sp-reorder-landing')!,
      path = node.querySelector('[data-reorder-landing-contour]')!;
    action({ landing, node, path, publication, bubbles, body });
  } catch (cause) {
    causes.push(cause);
  }
  try {
    landing.clear();
  } catch (cause) {
    causes.push(cause);
  }
  if (causes.length === 1) throw causes[0];
  if (causes.length) throw new AggregateError(causes, 'landing-refresh-proof-retirement');
}

test('unchanged stroke skips its actual setter while pending paint and changed or tampered colors remain live', () =>
  withLandingSurface(({ landing, node, path, publication, bubbles, body }) => {
    const d = path.getAttribute('d'),
      set = path.setAttribute;
    let writes = 0,
      forbid = true;
    path.setAttribute = (name, value) => {
      if (name === 'stroke') {
        writes++;
        if (forbid) throw new Error('redundant-stroke-write');
      }
      set.call(path, name, value);
    };
    assert.equal(
      publication.refresh(true, () => true),
      true,
    );
    assert.equal(writes, 0);
    assert.equal(node.style.opacity, '0.52');
    assert.equal(node.dataset.reorderLanding, 'pending');
    assert.equal(
      publication.refresh(false, () => true),
      true,
    );
    assert.equal(writes, 0);
    assert.equal(node.style.opacity, '0.82');
    assert.equal(node.dataset.reorderLanding, 'settled');
    assert.equal(path.getAttribute('d'), d);
    forbid = false;
    set.call(path, 'stroke', 'red');
    assert.equal(
      publication.refresh(false, () => true),
      true,
    );
    assert.equal(writes, 1);
    assert.equal(path.getAttribute('stroke'), 'black');
    const next = landing.prepare(bubbles, 'blue', false, viewport);
    assert.equal(
      next.commit(() => true),
      true,
    );
    assert.equal(writes, 2);
    assert.equal(body.querySelector('.sp-reorder-landing'), node);
    assert.equal(node.querySelector('[data-reorder-landing-contour]'), path);
    assert.equal(path.getAttribute('d'), d);
    assert.equal(path.getAttribute('stroke'), 'blue');
  }));

test('stroke read reentry cannot write into or withdraw the newer actual publication', () =>
  withLandingSurface(({ landing, node: old, path, bubbles, body }) => {
    const candidate = landing.prepare(bubbles, 'blue', false, viewport),
      get = path.getAttribute;
    let fired = false;
    path.getAttribute = (name) => {
      if (name === 'stroke' && !fired) {
        fired = true;
        landing.show(bubbles, 'green', false, viewport);
      }
      return get.call(path, name);
    };
    path.setAttribute = () => {
      throw new Error('stale-stroke-write');
    };
    assert.equal(
      candidate.commit(() => true),
      false,
    );
    assert.equal(fired, true);
    const current = body.querySelector('.sp-reorder-landing')!;
    assert.notEqual(current, old);
    assert.equal(old.isConnected, false);
    assert.equal(
      current.querySelector('[data-reorder-landing-contour]')!.getAttribute('stroke'),
      'green',
    );
    candidate.release();
    assert.equal(current.isConnected, true);
  }));

test('stroke read cancellation rejects before the actual setter', () =>
  withLandingSurface(({ path, publication }) => {
    const get = path.getAttribute;
    let allowed = true;
    path.getAttribute = (name) => {
      if (name === 'stroke') allowed = false;
      return get.call(path, name);
    };
    path.setAttribute = () => {
      throw new Error('cancelled-stroke-write');
    };
    assert.equal(
      publication.refresh(false, () => allowed),
      false,
    );
    assert.equal(publication.current(), true);
  }));

test('one-shot stroke-read rejection cannot recover into a successful changed-color publication', () =>
  withLandingSurface(({ landing, node, path, bubbles }) => {
    const candidate = landing.prepare(bubbles, 'blue', false, viewport),
      get = path.getAttribute;
    let rejectNext = false,
      rejected = 0,
      writes = 0;
    path.getAttribute = (name) => {
      if (name === 'stroke') rejectNext = true;
      return get.call(path, name);
    };
    path.setAttribute = () => {
      writes++;
    };
    assert.equal(
      candidate.commit(() => {
        if (rejectNext) {
          rejectNext = false;
          rejected++;
          return false;
        }
        return true;
      }),
      false,
    );
    assert.equal(rejected, 1);
    assert.equal(writes, 0);
    assert.equal(get.call(path, 'stroke'), 'black');
    assert.equal(candidate.current(), false);
    assert.equal(node.isConnected, false);
  }));

for (const raw of [null, undefined])
  test('failed stroke read retains its raw cause and retires the exact candidate', () =>
    withLandingSurface(({ landing, node, path, bubbles }) => {
      const candidate = landing.prepare(bubbles, 'blue', false, viewport),
        get = path.getAttribute;
      path.getAttribute = (name) => {
        // eslint-disable-next-line @typescript-eslint/only-throw-error -- Inject raw host failures to prove original cause retention and cleanup.
        if (name === 'stroke') throw raw;
        return get.call(path, name);
      };
      assert.throws(
        () => candidate.commit(() => true),
        (cause: unknown) => cause === raw,
      );
      assert.equal(candidate.current(), false);
      assert.equal(node.isConnected, false);
    }));
