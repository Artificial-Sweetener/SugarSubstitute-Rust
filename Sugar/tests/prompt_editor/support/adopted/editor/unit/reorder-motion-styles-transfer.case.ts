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
/** Whole stylesheet transfer proves retained physical publication and exact admission/retirement authority. */
import assert from 'node:assert/strict';
import test from 'node:test';
import { ReorderMotionStyles } from '../../../../../../frontend/sugarprompt/editor/src/reorder-motion-styles.ts';
import { reorderPaintIssueName } from '../../../../../../frontend/sugarprompt/editor/src/reorder-paint-identity.ts';
import { motionPaintFixture } from './reorder-motion-paint-fixture.ts';
import { motionStylesFixture as fixture } from './reorder-motion-styles-fixture.ts';

test('whole transfer reuses physical gate and sheet without native writes and carries the bounded roster', () => {
  const { f, owner, sheet, marker } = fixture(),
    old = {},
    next = {};
  try {
    owner.prepare([1], 'whole', () => true);
    owner.activate(old, () => true);
    const node = sheet(),
      gate = marker(),
      rules = node.textContent,
      set = gate.setAttribute.bind(gate),
      remove = gate.remove.bind(gate),
      sheetRemove = node.remove.bind(node),
      append = f.document.head!.append.bind(f.document.head!),
      insert = f.view.dom.insertBefore.bind(f.view.dom),
      create = f.document.createElement.bind(f.document);
    /** Successful transfer has no native allocation, publication or stylesheet mutation. */
    const forbidden = (): never => {
      throw new Error('whole-transfer-native-write');
    };
    Object.defineProperty(node, 'textContent', {
      configurable: true,
      get: () => rules,
      set: forbidden,
    });
    gate.setAttribute = forbidden;
    gate.remove = forbidden;
    node.remove = forbidden;
    f.document.head!.append = forbidden;
    f.view.dom.insertBefore = forbidden;
    f.document.createElement = forbidden;
    try {
      assert.equal(
        owner.transfer(old, next, [2, 3], () => true),
        true,
      );
      assert.equal(sheet(), node);
      assert.equal(marker(), gate);
      assert.equal(node.textContent, rules);
      owner.deactivate(old);
      assert.equal(
        gate.getAttribute('data-reorder-motion-active'),
        reorderPaintIssueName(owner.scope),
      );
    } finally {
      gate.setAttribute = set;
      gate.remove = remove;
      node.remove = sheetRemove;
      f.document.head!.append = append;
      f.view.dom.insertBefore = insert;
      f.document.createElement = create;
      Reflect.deleteProperty(node, 'textContent');
    }
    owner.deactivate(next);
    owner.prepare([4], 'native', () => true);
    for (const id of [1, 2, 3, 4])
      assert.equal(node.textContent!.includes(`data-reorder-chip="${id}"`), true);
  } finally {
    owner.retire();
  }
});

for (const policy of ['native', 'held'] as const)
  test(`${policy} gate cannot use whole-only transfer`, () => {
    const { owner, marker } = fixture(),
      issue = {};
    try {
      owner.prepare([1], policy, () => true);
      owner.activate(issue, () => true);
      assert.equal(
        owner.transfer(issue, {}, [2], () => true),
        false,
      );
      assert.equal(
        marker().getAttribute('data-reorder-motion-active'),
        reorderPaintIssueName(owner.scope),
      );
      owner.deactivate(issue);
    } finally {
      owner.retire();
    }
  });

for (const raw of [null, undefined])
  test(`whole transfer getter retains raw ${String(raw)} without committing another issue`, () => {
    const { owner, sheet, marker } = fixture(),
      issue = {};
    try {
      owner.prepare([1], 'whole', () => true);
      owner.activate(issue, () => true);
      const node = sheet();
      Object.defineProperty(node, 'textContent', {
        configurable: true,
        get() {
          // eslint-disable-next-line @typescript-eslint/only-throw-error -- Inject raw host failures to prove original cause retention and cleanup.
          throw raw;
        },
      });
      let caught = false;
      try {
        owner.transfer(issue, {}, [2], () => true);
      } catch (cause) {
        caught = true;
        assert.equal(cause, raw);
      }
      assert.equal(caught, true);
      assert.equal(
        marker().getAttribute('data-reorder-motion-active'),
        reorderPaintIssueName(owner.scope),
      );
      owner.deactivate(issue);
    } finally {
      owner.retire();
    }
  });

test('whole transfer reentrant retirement stops before consuming any later scope getter', () => {
  const { owner, sheet, marker } = fixture(),
    issue = {};
  try {
    owner.prepare([1], 'whole', () => true);
    owner.activate(issue, () => true);
    const node = sheet(),
      gate = marker(),
      rules = node.textContent,
      get = gate.getAttribute.bind(gate);
    Object.defineProperty(node, 'textContent', {
      configurable: true,
      get() {
        owner.retire();
        gate.getAttribute = () => {
          throw new Error('retired-scope-read');
        };
        return rules;
      },
    });
    assert.equal(
      owner.transfer(issue, {}, [2], () => true),
      false,
    );
    gate.getAttribute = get;
    assert.equal(node.isConnected, false);
    assert.equal(gate.isConnected, false);
  } finally {
    owner.retire();
  }
});

test('whole transfer admission reentry preserves the newer JavaScript issue', () => {
  const { owner, marker } = fixture(),
    old = {},
    stale = {},
    newer = {};
  try {
    owner.prepare([1], 'whole', () => true);
    owner.activate(old, () => true);
    let entered = false;
    assert.equal(
      owner.transfer(old, stale, [2], () => {
        if (!entered) {
          entered = true;
          assert.equal(
            owner.transfer(old, newer, [3], () => true),
            true,
          );
        }
        return true;
      }),
      false,
    );
    owner.deactivate(old);
    owner.deactivate(stale);
    assert.equal(
      marker().getAttribute('data-reorder-motion-active'),
      reorderPaintIssueName(owner.scope),
    );
    owner.deactivate(newer);
    assert.equal(marker().getAttribute('data-reorder-motion-active'), '');
  } finally {
    owner.retire();
  }
});

test('whole transfer retains exact ShadowRoot attachment authority', () => {
  const f = motionPaintFixture(),
    root = f.document.createElement('shadow-root');
  f.document.body!.append(root);
  Object.defineProperty(f.view, 'root', { value: root });
  const create = f.document.createElement.bind(f.document);
  f.document.createElement = (name) => {
    const node = create(name);
    node.getRootNode = () => root;
    return node;
  };
  const owner = new ReorderMotionStyles(f.view),
    old = {},
    next = {};
  try {
    assert.equal(
      owner.prepare([1], 'whole', () => true),
      true,
    );
    assert.equal(
      owner.activate(old, () => true),
      true,
    );
    const sheet = root.querySelector('style');
    assert.ok(sheet);
    assert.equal(
      owner.transfer(old, next, [2], () => true),
      true,
    );
    assert.equal(root.querySelector('style'), sheet);
    owner.deactivate(old);
    assert.equal(
      f.view.dom
        .querySelector('[data-reorder-motion-gate]')!
        .getAttribute('data-reorder-motion-active'),
      reorderPaintIssueName(owner.scope),
    );
    owner.deactivate(next);
  } finally {
    owner.retire();
  }
  assert.equal(root.childNodes.length, 0);
});
