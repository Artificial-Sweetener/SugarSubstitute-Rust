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
/** Collision calibration borrows one containing line; whole paint independently admits its complete native extent. */
import assert from 'node:assert/strict';
import test from 'node:test';
import { hideReorderChip } from '../../../../../../frontend/sugarprompt/core/src/index.ts';
import { readReorderBaseContext } from '../../../../../../frontend/sugarprompt/editor/src/reorder-base-context.ts';
import { ReorderCollisionBase } from '../../../../../../frontend/sugarprompt/editor/src/reorder-collision-base.ts';
import type { ReorderMeasureInput } from '../../../../../../frontend/sugarprompt/editor/src/reorder-measurement-contracts.ts';
import { nativeFixture } from './reorder-native-projection-fixture.ts';

/** Three real Core chips keep the fixture's moved publication valid across the requested hard-line count. */
function multilineSource(lines: number): string {
  return 'a, (' + Array.from({ length: lines }, () => 'b').join('\n') + '), d';
}

for (const lines of [16, 17, 80]) {
  test(`collision context admits ${lines} native hard lines without reading their union`, () => {
    const t = nativeFixture(multilineSource(lines)),
      base = new ReorderCollisionBase(t.f.view);
    try {
      const publication = t.publish(),
        state = t.state,
        hidden = hideReorderChip(t.pickup.capture.prepared, t.pickup.order, 0);
      assert.ok(hidden);
      assert.equal(state.doc.lines, lines);
      assert.equal(publication.pointer?.mode, 'stage');
      const input: ReorderMeasureInput = {
        token: t.pickup.token,
        capture: t.pickup.capture,
        order: t.pickup.order,
        held: 0,
        hidden,
        pointer: 9,
        coordinates: { x: 55, y: 50 },
      };
      let reads = 0;
      t.f.view.domAtPos = (position) => {
        assert.equal(++reads, 1, 'collision must not enumerate native hard lines');
        assert.equal(position, state.doc.line(1).from);
        return { node: t.f.line as unknown as Node, offset: 0 };
      };
      assert.equal(base.prepare(input), true);
      const context = base.contextAt(input);
      assert.ok(context);
      assert.equal(context.width, 200);
      assert.equal(context.height, 20);
      assert.equal(context.current(), true);
      assert.equal(reads, 1);
      assert.equal(t.state, state);
    } finally {
      try {
        base.destroy();
      } finally {
        t.owner.destroy();
      }
    }
  });

  test(`whole pointer paint retains its exact sixteen-line extent bound for ${lines} lines`, () => {
    const t = nativeFixture(multilineSource(lines));
    try {
      const publication = t.publish(),
        state = t.state;
      assert.equal(publication.pointer?.mode, 'stage');
      assert.ok(publication.value);
      const nativeLines = [t.f.line];
      if (lines === 16) {
        for (let index = 1; index < lines; index++) {
          const line = t.f.document.createElement('div');
          line.className = 'cm-line';
          Object.assign(line.box, {
            left: 10,
            top: 20 + index * 20,
            width: 200,
            height: 20,
          });
          t.f.content.append(line);
          nativeLines.push(line);
        }
      }
      let reads = 0;
      t.f.view.domAtPos = (position) => {
        reads++;
        if (lines !== 16)
          assert.equal(reads, 1, 'oversized whole paint refuses before enumeration');
        const line = nativeLines[state.doc.lineAt(position).number - 1];
        assert.ok(line);
        return { node: line as unknown as Node, offset: 0 };
      };
      const context = readReorderBaseContext(t.f.view, publication.value);
      if (lines === 16) {
        assert.ok(context);
        assert.equal(context.height, 320);
        assert.equal(context.width, 200);
        assert.equal(context.current(), true);
        assert.equal(reads, 16);
        nativeLines[15]!.remove();
        assert.equal(context.current(), false);
      } else {
        assert.equal(context, null);
        assert.equal(reads, 1);
      }
      assert.equal(t.state, state);
    } finally {
      t.owner.destroy();
    }
  });
}

test('collision context refuses a supported position without a containing native line', () => {
  const t = nativeFixture(multilineSource(80)),
    base = new ReorderCollisionBase(t.f.view);
  try {
    t.publish();
    const hidden = hideReorderChip(t.pickup.capture.prepared, t.pickup.order, 0);
    assert.ok(hidden);
    const input: ReorderMeasureInput = {
        token: t.pickup.token,
        capture: t.pickup.capture,
        order: t.pickup.order,
        held: 0,
        hidden,
        pointer: 9,
        coordinates: { x: 55, y: 50 },
      },
      placeholder = t.f.document.createElement('span');
    t.f.view.domAtPos = () => ({
      node: placeholder as unknown as Node,
      offset: 0,
    });
    assert.equal(base.prepare(input), true);
    assert.equal(base.contextAt(input), null);
  } finally {
    try {
      base.destroy();
    } finally {
      t.owner.destroy();
    }
  }
});
