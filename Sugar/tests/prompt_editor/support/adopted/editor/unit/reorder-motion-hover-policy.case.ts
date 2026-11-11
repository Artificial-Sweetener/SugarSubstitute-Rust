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
/** Actual stylesheet and presentation owners consume the existing pointer policy without another hover or gesture state. */
import assert from 'node:assert/strict';
import test from 'node:test';
import { motionStylesFixture } from './reorder-motion-styles-fixture.ts';
import { motionPresentationFixture } from './reorder-motion-presentation-fixture.ts';

test('native to held to native with equal IDs retains the same sheet and restores exact native rule bytes', () => {
  const { owner, sheet, marker } = motionStylesFixture();
  try {
    assert.equal(
      owner.prepare([1, 2], 'native', () => true),
      true,
    );
    const node = sheet(),
      gate = marker(),
      native = node.textContent;
    for (const hover of ['held', 'native'] as const) {
      assert.equal(
        owner.prepare([2, 1], hover, () => true),
        true,
      );
      assert.equal(sheet() === node, true);
      assert.equal(marker() === gate, true);
      const rules = node.textContent ?? '';
      assert.equal(rules.includes(':has('), hover === 'native');
      assert.equal(rules.includes(':not(:hover)'), hover === 'native');
      assert.equal(rules.includes('visibility:hidden'), hover === 'native');
      assert.equal(rules.includes('{opacity:0!important}'), true);
      if (hover === 'native') assert.equal(rules === native, true);
      const issue = {};
      assert.equal(
        owner.activate(issue, () => true),
        true,
      );
      owner.deactivate(issue);
    }
  } finally {
    owner.retire();
  }
});

test('changing held IDs retains actual sheet declarations without assigning identical bytes and retains the bounded native-return roster', () => {
  const { owner, sheet, marker } = motionStylesFixture();
  try {
    assert.equal(
      owner.prepare([1], 'held', () => true),
      true,
    );
    const node = sheet(),
      gate = marker(),
      rules = node.textContent,
      descriptor = Object.getOwnPropertyDescriptor(node, 'textContent');
    Object.defineProperty(node, 'textContent', {
      configurable: true,
      get: () => rules,
      set() {
        throw new Error('held-equal-output-must-not-rewrite');
      },
    });
    assert.equal(
      owner.prepare([2], 'held', () => true),
      true,
    );
    assert.equal(sheet() === node && marker() === gate, true);
    assert.equal(node.textContent === rules, true);
    const issue = {};
    assert.equal(
      owner.activate(issue, () => true),
      true,
    );
    owner.deactivate(issue);
    if (descriptor) Object.defineProperty(node, 'textContent', descriptor);
    else Reflect.deleteProperty(node, 'textContent');
    assert.equal(
      owner.prepare([2], 'native', () => true),
      true,
    );
    assert.equal(node.textContent!.includes('data-reorder-chip="1"'), true);
    assert.equal(node.textContent!.includes('data-reorder-chip="2"'), true);
  } finally {
    owner.retire();
  }
});

test('policy publication reentry cannot restore an older native rule artifact or disable the newer held issue', () => {
  const { owner, sheet, marker, f } = motionStylesFixture(),
    oldIssue = {},
    newerIssue = {};
  try {
    let entered = false;
    assert.equal(
      owner.prepare([1], 'native', () => {
        if (!entered && f.document.head!.childNodes.length > 0) {
          entered = true;
          assert.equal(
            owner.prepare([1], 'held', () => true),
            true,
          );
          assert.equal(
            owner.activate(newerIssue, () => true),
            true,
          );
        }
        return true;
      }),
      false,
    );
    assert.equal(entered, true);
    assert.equal(sheet().textContent!.includes(':has('), false);
    const active = marker().getAttribute('data-reorder-motion-active');
    owner.deactivate(oldIssue);
    assert.equal(marker().getAttribute('data-reorder-motion-active') === active, true);
    owner.deactivate(newerIssue);
    assert.equal(marker().getAttribute('data-reorder-motion-active'), '');
  } finally {
    owner.retire();
  }
});

for (const raw of [null, undefined])
  test(`held policy rewrite preserves raw ${String(raw)} and retires the exact sheet and marker`, () => {
    const { owner, sheet, marker, f } = motionStylesFixture();
    try {
      assert.equal(
        owner.prepare([1], 'native', () => true),
        true,
      );
      const node = sheet(),
        gate = marker();
      Object.defineProperty(node, 'textContent', {
        configurable: true,
        set() {
          // eslint-disable-next-line @typescript-eslint/only-throw-error -- Inject raw host failures to prove original cause retention and cleanup.
          throw raw;
        },
      });
      let caught = false;
      try {
        owner.prepare([1], 'held', () => true);
      } catch (cause) {
        caught = true;
        assert.equal(cause === raw, true);
      }
      assert.equal(caught, true);
      assert.equal(owner.available, false);
      assert.equal(node.isConnected || gate.isConnected, false);
      assert.equal(f.document.head!.childNodes.length, 0);
    } finally {
      owner.retire();
    }
  });

for (const pointer of [null, 17])
  test(`actual presentation routes ${pointer === null ? 'native' : 'held'} policy from the existing pointer input`, () => {
    const f = motionPresentationFixture();
    try {
      Object.assign(f.input, { pointer });
      f.prepare();
      f.deliver();
      const sheets = f.document
        .head!.querySelectorAll('style')
        .filter((node) => node.textContent.includes('data-reorder-motion-local'));
      assert.equal(sheets.length, 1);
      const rules = sheets[0]!.textContent;
      assert.equal(rules.includes(':has('), pointer === null);
      assert.equal(rules.includes(':not(:hover)'), pointer === null);
    } finally {
      f.destroy();
    }
  });
