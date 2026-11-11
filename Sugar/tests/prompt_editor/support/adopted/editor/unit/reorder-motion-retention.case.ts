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
/** Presentation alone owns exact accepted/pending no-origin capabilities across native phases. */
import assert from 'node:assert/strict';
import test from 'node:test';
import { motionPresentationFixture } from './reorder-motion-presentation-fixture.ts';

test('pending capability rejects completion and replacement even for the same mutable input', () => {
  const f = motionPresentationFixture();
  try {
    f.prepare();
    const pending = f.presentation.retain(f.input);
    assert.ok(pending);
    assert.equal(pending(), true);
    f.deliver();
    assert.equal(pending(), false);
    const accepted = f.presentation.retain(f.input);
    assert.ok(accepted);
    assert.equal(accepted(), true);
    f.presentation.cancel();
    assert.equal(accepted(), false);
    assert.equal(f.presentation.retain(f.input), null);
    f.prepare();
    assert.ok(f.jobs.length);
    assert.equal(pending(), false);
    f.deliver();
    assert.ok(f.presentation.retain(f.input));
  } finally {
    f.destroy();
  }
});

test('same-source publication rejects the prior issued capability before a new request is admitted', () => {
  const f = motionPresentationFixture();
  try {
    f.prepare();
    const retained = f.presentation.retain(f.input);
    assert.ok(retained);
    f.publish();
    assert.equal(retained(), false);
    assert.equal(f.presentation.retain(f.input), null);
    f.prepare();
    assert.equal(retained(), false);
    assert.ok(f.presentation.retain(f.input));
  } finally {
    f.destroy();
  }
});
