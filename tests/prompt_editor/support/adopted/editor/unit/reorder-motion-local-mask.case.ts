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
import { reorderPreview } from '../../../../../../frontend/sugarprompt/editor/src/reorder-preview.ts';
/** Local activation and bounded prepared-rule reuse preserve native marker ownership independently of CM visibility authority. */
import assert from 'node:assert/strict';
import test from 'node:test';
import { motionStylesFixture as fixture } from './reorder-motion-styles-fixture.ts';
import { reorderPaintIssueName } from '../../../../../../frontend/sugarprompt/editor/src/reorder-paint-identity.ts';
test('prepared subset reuses actual rule bytes and local marker without any stylesheet media access', () => {
  const { owner, sheet, marker, f } = fixture();
  const declarations: (readonly [string, string, string | undefined])[] = [];
  const create = f.document.createElement.bind(f.document);
  f.document.createElement = (name) => {
    const node = create(name),
      set = node.style.setProperty.bind(node.style);
    node.style.setProperty = (name, value, priority) => {
      declarations.push([name, value, priority]);
      set(name, value, priority);
    };
    return node;
  };
  try {
    owner.prepare([1, 2, 3], 'native', () => true);
    const node = sheet(),
      gate = marker(),
      rules = node.textContent;
    Object.defineProperty(node, 'media', {
      get() {
        throw new Error('unexpected-media-read');
      },
      set() {
        throw new Error('unexpected-media-write');
      },
    });
    Object.defineProperty(node, 'textContent', {
      get: () => rules,
      set() {
        throw new Error('unexpected-prepared-subset-rewrite');
      },
    });
    assert.equal(
      owner.prepare([3, 1], 'native', () => true),
      true,
    );
    assert.equal(sheet(), node);
    assert.equal(marker(), gate);
    assert.equal(node.textContent, rules);
    assert.equal(gate.parentElement, f.view.dom);
    assert.equal(gate.getRootNode(), owner.root);
    assert.equal(Reflect.get(gate.style, 'display'), 'none');
    assert.deepEqual(declarations, [['display', 'none', 'important']]);
    assert.equal(gate.inert, true);
    assert.equal(gate.hidden, true);
    assert.equal(gate.getAttribute('aria-hidden'), 'true');
    const issue = {};
    assert.equal(
      owner.activate(issue, () => true),
      true,
    );
    assert.equal(
      gate.getAttribute('data-reorder-motion-active'),
      reorderPaintIssueName(owner.scope),
    );
    owner.deactivate(issue);
    assert.equal(gate.getAttribute('data-reorder-motion-active'), '');
  } finally {
    owner.retire();
  }
  assert.equal(f.view.dom.querySelector('[data-reorder-motion-gate]'), null);
});

test('union overflow replaces its actual sheet rules with the exact valid selected subset', () => {
  const { owner, sheet } = fixture();
  try {
    owner.prepare(
      Array.from({ length: 64 }, (_, i) => i),
      'native',
      () => true,
    );
    const node = sheet();
    assert.equal(
      owner.prepare([64, 65], 'native', () => true),
      true,
    );
    assert.equal(sheet(), node);
    const ids = [
      ...new Set(
        [...node.textContent!.matchAll(/data-reorder-chip="(\d+)"/g)].map((match) =>
          Number(match[1]),
        ),
      ),
    ];
    assert.deepEqual(ids, [64, 65]);
    const rules = node.textContent;
    Object.defineProperty(node, 'textContent', {
      get: () => rules,
      set() {
        throw new Error('unexpected-subset-rewrite');
      },
    });
    assert.equal(
      owner.prepare([65], 'native', () => true),
      true,
    );
    assert.equal(sheet(), node);
  } finally {
    owner.retire();
  }
});

for (const primary of [null, undefined])
  test(`partial local marker acquisition preserves raw ${String(primary)} and independently retires sheet and marker`, () => {
    const { f, owner } = fixture(),
      create = f.document.createElement.bind(f.document);
    let marker: ReturnType<typeof create> | null = null;
    f.document.createElement = (name) => {
      const node = create(name);
      if (name === 'span') {
        marker = node;
        node.setAttribute = () => {
          // eslint-disable-next-line @typescript-eslint/only-throw-error -- Inject raw host failures to prove original cause retention and cleanup.
          throw primary;
        };
      }
      return node;
    };
    const failures: unknown[] = [];
    try {
      owner.prepare([1], 'native', () => true);
    } catch (cause) {
      failures.push(cause);
    }
    assert.equal(failures.length, 1);
    const failure = failures[0];
    assert.ok(failure instanceof AggregateError);
    assert.deepEqual(failure.errors, [primary, primary]);
    assert.equal(owner.available, false);
    assert.equal(f.document.head!.childNodes.length, 0);
    assert.ok(marker);
    assert.equal(Reflect.get(marker, 'isConnected'), false);
    assert.equal(f.view.dom.querySelector('[data-reorder-motion-gate]'), null);
    owner.retire();
  });

test('partial marker initialization reentry replaces incomplete output before a newer exact activation', () => {
  const { f, owner, sheet, marker } = fixture(),
    create = f.document.createElement.bind(f.document);
  let entered = false,
    partial: ReturnType<typeof create> | null = null;
  f.document.createElement = (name) => {
    const node = create(name);
    if (name === 'span' && !entered) {
      partial = node;
      const set = node.setAttribute.bind(node);
      node.setAttribute = (name, value) => {
        set(name, value);
        if (name === 'aria-hidden' && !entered) {
          entered = true;
          assert.equal(
            owner.prepare([2], 'native', () => true),
            true,
          );
        }
      };
    }
    return node;
  };
  try {
    assert.equal(
      owner.prepare([1], 'native', () => true),
      false,
    );
    assert.ok(partial);
    assert.equal(Reflect.get(partial, 'isConnected'), false);
    const gate = marker();
    assert.notEqual(gate, partial);
    assert.equal(gate.getAttribute('aria-hidden'), 'true');
    assert.equal(gate.hidden, true);
    assert.equal(gate.inert, true);
    assert.equal(gate.getAttribute('data-reorder-motion-gate'), reorderPaintIssueName(owner.scope));
    assert.equal(sheet().textContent!.includes('data-reorder-chip="2"'), true);
    assert.equal(sheet().textContent!.includes('data-reorder-chip="1"'), false);
    assert.equal(
      owner.activate({}, () => true),
      true,
    );
    assert.equal(
      gate.getAttribute('data-reorder-motion-active'),
      reorderPaintIssueName(owner.scope),
    );
  } finally {
    owner.retire();
  }
});

for (const boundary of ['admission', 'write'] as const)
  test(`detached marker at activation ${boundary} cannot report an active native mask`, () => {
    const { owner, marker } = fixture(),
      issue = {};
    try {
      owner.prepare([1], 'native', () => true);
      const gate = marker();
      const set = gate.setAttribute.bind(gate);
      let entered = false;
      if (boundary === 'write')
        gate.setAttribute = (name, value) => {
          set(name, value);
          if (name === 'data-reorder-motion-active' && value) gate.remove();
        };
      assert.equal(
        owner.activate(issue, () => {
          if (boundary === 'admission' && !entered) {
            entered = true;
            gate.remove();
          }
          return true;
        }),
        false,
      );
      assert.notEqual(
        gate.getAttribute('data-reorder-motion-active'),
        reorderPaintIssueName(owner.scope),
      );
      gate.setAttribute = set;
      assert.equal(
        owner.prepare([1], 'native', () => true),
        true,
      );
      assert.equal(marker(), gate);
      assert.equal(
        owner.activate({}, () => true),
        true,
      );
    } finally {
      owner.retire();
    }
  });
import { motionPresentationFixture } from './reorder-motion-presentation-fixture.ts';

/** Actual canonical Stage belongs to the native layer marker; only the activation gate precedes existing editor children. */
test('sibling gate precedes native layer Stage without moving managed content', () => {
  const f = motionPresentationFixture();
  try {
    const children = [...f.view.dom.childNodes],
      contentChildren = [...f.view.contentDOM.childNodes];
    f.prepare();
    f.deliver();
    const gate = f.view.dom.querySelector('[data-reorder-motion-gate]'),
      stage = f.view.dom.querySelector('.sp-reorder-motion-stage'),
      paint = f.view.dom.querySelector('.sp-reorder-motion-paint');
    assert.ok(gate);
    assert.ok(stage);
    assert.ok(paint);
    assert.equal(f.view.dom.childNodes[0], gate);
    assert.equal(stage.parentElement, f.paint.layerMount);
    assert.equal(gate.parentElement, f.view.dom);
    assert.equal(stage.contains(paint), true);
    assert.deepEqual(
      [...f.view.dom.childNodes].filter((node) => node !== gate),
      children,
    );
    assert.deepEqual([...f.view.contentDOM.childNodes], contentChildren);
    const mask = f.state.field(reorderPreview).mask;
    assert.ok(mask);
    assert.equal(
      gate.getAttribute('data-reorder-motion-active'),
      reorderPaintIssueName(mask.scope),
    );
    f.presentation.cancel();
    assert.equal(gate.isConnected, false);
    assert.equal(stage.isConnected, false);
    assert.deepEqual([...f.view.dom.childNodes], children);
    assert.deepEqual([...f.view.contentDOM.childNodes], contentChildren);
  } finally {
    f.destroy();
  }
});

for (const boundary of ['admission', 'write'] as const)
  test(`marker displaced from first child at ${boundary} refuses activation and only repositions its own output`, () => {
    const { f, owner, marker } = fixture(),
      issue = {};
    try {
      const managed = [...f.view.dom.childNodes],
        content = [...f.view.contentDOM.childNodes];
      owner.prepare([1], 'native', () => true);
      const gate = marker(),
        prefix = f.view.dom.ownerDocument.createElement('span');
      assert.equal(f.view.dom.childNodes[0], gate);
      const set = gate.setAttribute.bind(gate);
      let entered = false;
      if (boundary === 'write')
        gate.setAttribute = (name, value) => {
          set(name, value);
          if (name === 'data-reorder-motion-active' && value) f.view.dom.insertBefore(prefix, gate);
        };
      assert.equal(
        owner.activate(issue, () => {
          if (boundary === 'admission' && !entered) {
            entered = true;
            f.view.dom.insertBefore(prefix, gate);
          }
          return true;
        }),
        false,
      );
      assert.notEqual(
        gate.getAttribute('data-reorder-motion-active'),
        reorderPaintIssueName(owner.scope),
      );
      assert.deepEqual(
        [...f.view.dom.childNodes].filter((node) => node !== gate),
        [prefix, ...managed],
      );
      gate.setAttribute = set;
      const insert = f.view.dom.insertBefore.bind(f.view.dom),
        moved: Node[] = [];
      f.view.dom.insertBefore = (node, reference) => {
        assert.equal(node, gate);
        moved.push(node);
        return insert(node, reference);
      };
      assert.equal(
        owner.prepare([1], 'native', () => true),
        true,
      );
      assert.equal(marker(), gate);
      assert.equal(f.view.dom.childNodes[0], gate);
      assert.deepEqual(moved, [gate]);
      assert.deepEqual(
        [...f.view.dom.childNodes].filter((node) => node !== gate),
        [prefix, ...managed],
      );
      assert.deepEqual([...f.view.contentDOM.childNodes], content);
      assert.equal(
        owner.activate({}, () => true),
        true,
      );
    } finally {
      owner.retire();
    }
  });
