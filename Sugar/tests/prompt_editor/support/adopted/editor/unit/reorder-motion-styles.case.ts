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
/** The actual finite stylesheet owner uses only controlled native allocation/activation-marker/append boundaries. */
import assert from 'node:assert/strict';
import test from 'node:test';
import { ReorderMotionStyles } from '../../../../../../frontend/sugarprompt/editor/src/reorder-motion-styles.ts';
import { reorderPaintIssueName } from '../../../../../../frontend/sugarprompt/editor/src/reorder-paint-identity.ts';
import { motionPaintFixture } from './reorder-motion-paint-fixture.ts';
import { motionStylesFixture as fixture } from './reorder-motion-styles-fixture.ts';

test('equal bounded selected rosters retain the actual sheet and rule bytes across exact issues', () => {
  const { f, owner, sheet, marker } = fixture(),
    first = {},
    second = {};
  try {
    const append = f.document.head!.append.bind(f.document.head!);
    f.document.head!.append = (...nodes) => {
      assert.equal(f.view.dom.querySelector('[data-reorder-motion-active]'), null);
      append(...nodes);
    };
    assert.equal(
      owner.prepare([2, 1], 'native', () => true),
      true,
    );
    const node = sheet(),
      rules = node.textContent,
      scope = reorderPaintIssueName(owner.scope),
      gate = `> [data-reorder-motion-gate="${scope}"][data-reorder-motion-active="${scope}"] ~`;
    assert.equal(
      rules,
      `.cm-editor ${gate} .cm-scroller > .cm-content [data-reorder-motion-local="${scope}"][data-reorder-chip]:not(:hover){opacity:0!important}\n.cm-editor ${gate} .cm-scroller:has(> .cm-content [data-reorder-motion-local="${scope}"][data-reorder-chip="1"]:hover) > .sp-reorder-motion-layer > .sp-reorder-motion-mount > .sp-reorder-motion-stage .sp-reorder-motion-paint [data-reorder-motion-chip="1"]{visibility:hidden!important}\n.cm-editor ${gate} .cm-scroller:has(> .cm-content [data-reorder-motion-local="${scope}"][data-reorder-chip="2"]:hover) > .sp-reorder-motion-layer > .sp-reorder-motion-mount > .sp-reorder-motion-stage .sp-reorder-motion-paint [data-reorder-motion-chip="2"]{visibility:hidden!important}`,
    );
    assert.equal(
      owner.activate(first, () => true),
      true,
    );
    assert.equal(
      marker().getAttribute('data-reorder-motion-active'),
      reorderPaintIssueName(owner.scope),
    );
    owner.deactivate(first);
    assert.equal(marker().getAttribute('data-reorder-motion-active') || '', '');
    Object.defineProperty(node, 'textContent', {
      configurable: true,
      get: () => rules,
      set() {
        throw new Error('equal-roster-rule-rewrite');
      },
    });
    assert.equal(
      owner.prepare([1, 2], 'native', () => true),
      true,
    );
    assert.equal(sheet(), node);
    assert.equal(sheet().textContent, rules);
    assert.equal(
      owner.activate(second, () => true),
      true,
    );
    owner.deactivate(first);
    assert.equal(
      marker().getAttribute('data-reorder-motion-active'),
      reorderPaintIssueName(owner.scope),
    );
    owner.deactivate(second);
    assert.equal(marker().getAttribute('data-reorder-motion-active') || '', '');
  } finally {
    owner.retire();
  }
  assert.equal(f.document.head!.childNodes.length, 0);
});

test('changed selected roster extends the bounded prepared union on the same inactive sheet', () => {
  const { owner, sheet, marker } = fixture();
  try {
    owner.prepare([0, 2], 'native', () => true);
    const node = sheet(),
      old = node.textContent;
    owner.prepare([1], 'native', () => true);
    assert.equal(sheet(), node);
    assert.notEqual(node.textContent, old);
    assert.equal(marker().getAttribute('data-reorder-motion-active') || '', '');
    assert.equal(node.textContent!.includes('data-reorder-chip="1"'), true);
    assert.equal(node.textContent!.includes('data-reorder-chip="0"'), true);
    assert.equal(node.textContent!.includes('data-reorder-chip="2"'), true);
  } finally {
    owner.retire();
  }
});

test('scope identities isolate independent editors and the ShadowRoot append boundary uses its exact root', () => {
  const first = fixture(),
    second = fixture(),
    shadow = motionPaintFixture();
  const root = {
    append: shadow.document.head!.append.bind(shadow.document.head!),
  };
  Object.defineProperty(shadow.view, 'root', { value: root });
  const create = shadow.document.createElement.bind(shadow.document);
  shadow.document.createElement = (name) => {
    const node = create(name);
    node.getRootNode = () => root;
    return node;
  };
  const third = new ReorderMotionStyles(shadow.view);
  try {
    first.owner.prepare([1], 'native', () => true);
    second.owner.prepare([1], 'native', () => true);
    third.prepare([1], 'native', () => true);
    assert.notEqual(first.owner.scope, second.owner.scope);
    assert.notEqual(first.sheet().textContent, second.sheet().textContent);
    assert.equal(third.root, root);
    assert.equal(shadow.document.head!.childNodes.length, 1);
    first.owner.retire();
    assert.equal(second.sheet().isConnected, true);
  } finally {
    first.owner.retire();
    second.owner.retire();
    third.retire();
  }
});

test('oversized, duplicate and invalid selected rosters refuse before native allocation', () => {
  const { f, owner } = fixture(),
    allocations = f.allocations;
  try {
    for (const ids of [Array.from({ length: 65 }, (_, index) => index), [1, 1], [-1], [0.5], [NaN]])
      assert.throws(() => owner.prepare(ids, 'native', () => true), TypeError);
    assert.equal(f.allocations, allocations);
    assert.equal(f.document.head!.childNodes.length, 0);
  } finally {
    owner.retire();
  }
});

test('activation admission reentry preserves the newer exact issue on the retained same sheet', () => {
  const { owner, sheet, marker } = fixture(),
    old = {},
    newer = {};
  try {
    owner.prepare([1], 'native', () => true);
    const node = sheet();
    let entered = false;
    assert.equal(
      owner.activate(old, () => {
        if (!entered) {
          entered = true;
          assert.equal(
            owner.prepare([2], 'native', () => true),
            true,
          );
          assert.equal(
            owner.activate(newer, () => true),
            true,
          );
        }
        return true;
      }),
      false,
    );
    assert.equal(sheet(), node);
    assert.equal(
      marker().getAttribute('data-reorder-motion-active'),
      reorderPaintIssueName(owner.scope),
    );
    assert.equal(node.textContent!.includes('data-reorder-chip="2"'), true);
    owner.deactivate(old);
    assert.equal(
      marker().getAttribute('data-reorder-motion-active'),
      reorderPaintIssueName(owner.scope),
    );
    owner.deactivate(newer);
    assert.equal(marker().getAttribute('data-reorder-motion-active') || '', '');
  } finally {
    owner.retire();
  }
});

test('final preparation admission reentry cannot return older success or overwrite newer rules', () => {
  const { owner, sheet, f } = fixture();
  try {
    let entered = false;
    assert.equal(
      owner.prepare([1], 'native', () => {
        if (!entered && owner.available) {
          const connected = f.document.head!.childNodes.length > 0;
          if (connected) {
            entered = true;
            assert.equal(
              owner.prepare([2], 'native', () => true),
              true,
            );
          }
        }
        return true;
      }),
      false,
    );
    assert.equal(entered, true);
    assert.equal(sheet().textContent!.includes('data-reorder-chip="2"'), true);
  } finally {
    owner.retire();
  }
});

for (const primary of [null, undefined]) {
  for (const boundary of ['create', 'text', 'append', 'activate'] as const)
    test(`${boundary} failure retains raw ${String(primary)} and retires its exact partial sheet`, () => {
      const { f, owner } = fixture();
      const create = f.document.createElement.bind(f.document);
      f.document.createElement = (name) => {
        // eslint-disable-next-line @typescript-eslint/only-throw-error -- Inject raw host failures to prove original cause retention and cleanup.
        if (boundary === 'create') throw primary;
        const node = create(name);
        if (boundary === 'text')
          Object.defineProperty(node, 'textContent', {
            configurable: true,
            set() {
              // eslint-disable-next-line @typescript-eslint/only-throw-error -- Inject raw host failures to prove original cause retention and cleanup.
              throw primary;
            },
          });
        return node;
      };
      if (boundary === 'append')
        f.document.head!.append = () => {
          // eslint-disable-next-line @typescript-eslint/only-throw-error -- Inject raw host failures to prove original cause retention and cleanup.
          throw primary;
        };
      try {
        const causes: unknown[] = [];
        try {
          owner.prepare([1], 'native', () => true);
          if (boundary === 'activate') {
            const node = f.view.dom.querySelector('[data-reorder-motion-gate]');
            assert.ok(node);
            const set = node.setAttribute.bind(node);
            node.setAttribute = (name, value) => {
              // eslint-disable-next-line @typescript-eslint/only-throw-error -- Inject raw host failures to prove original cause retention and cleanup.
              if (name === 'data-reorder-motion-active' && value) throw primary;
              set(name, value);
            };
            owner.activate({}, () => true);
          }
        } catch (cause) {
          causes.push(cause);
        }
        assert.deepEqual(causes, [primary]);
        assert.equal(owner.available, false);
        assert.equal(f.document.head!.childNodes.length, 0);
      } finally {
        owner.retire();
      }
    });

  test(`deactivation raw ${String(primary)} independently removes the enabled sheet before scene retirement`, () => {
    const { owner, sheet, marker, f } = fixture(),
      issue = {};
    try {
      owner.prepare([1], 'native', () => true);
      owner.activate(issue, () => true);
      const node = sheet();
      const gate = marker();
      const set = gate.setAttribute.bind(gate);
      gate.setAttribute = (name, value) => {
        // eslint-disable-next-line @typescript-eslint/only-throw-error -- Inject raw host failures to prove original cause retention and cleanup.
        if (name === 'data-reorder-motion-active') throw primary;
        set(name, value);
      };
      assert.throws(
        () => owner.deactivate(issue),
        (cause) =>
          cause instanceof AggregateError &&
          cause.errors.length === 2 &&
          cause.errors[0] === primary &&
          cause.errors[1] === primary,
      );
      assert.equal(node.isConnected, false);
      assert.equal(f.document.head!.childNodes.length, 0);
      assert.equal(owner.available, false);
      owner.retire();
    } finally {
      owner.retire();
    }
  });
}

test("append cancellation and removal reentry cannot retain an old sheet or remove a newer owner's sheet", () => {
  const { f, owner } = fixture();
  const append = f.document.head!.append.bind(f.document.head!);
  f.document.head!.append = (...nodes) => {
    append(...nodes);
    owner.retire();
  };
  assert.equal(
    owner.prepare([1], 'native', () => true),
    false,
  );
  assert.equal(f.document.head!.childNodes.length, 0);
  f.document.head!.append = append;
  const second = new ReorderMotionStyles(f.view),
    newer = new ReorderMotionStyles(f.view);
  try {
    second.prepare([1], 'native', () => true);
    const node = f.document.head!.childNodes[0]!;
    assert.ok('remove' in node);
    const remove = node.remove.bind(node);
    node.remove = () => {
      remove();
      newer.prepare([2], 'native', () => true);
      newer.activate({}, () => true);
    };
    second.retire();
    assert.equal(f.document.head!.childNodes.length, 1);
    const gate = f.view.dom.querySelector('[data-reorder-motion-gate]');
    assert.ok(gate);
    assert.equal(
      gate.getAttribute('data-reorder-motion-active'),
      reorderPaintIssueName(newer.scope),
    );
  } finally {
    second.retire();
    newer.retire();
  }
});

/** Native masks select actual scope/current IDs and become inert as soon as the issuing field removes its root scope gate. */
test('native source rules require the exact root value and current mask scope without mutating chips', () => {
  const { f, owner, sheet } = fixture(),
    causes: unknown[] = [];
  try {
    f.view.dom.setAttribute('data-reorder-native-value', '2');
    assert.equal(
      owner.prepare([1, 2], 'held', () => true),
      true,
    );
    const first = sheet(),
      scope = reorderPaintIssueName(owner.scope);
    assert.ok(
      first.textContent!.startsWith(
        '.cm-editor[data-reorder-native-value="2"][data-reorder-motion-scope="' + scope + '"]',
      ),
    );
    assert.ok(
      first.textContent!.includes('[data-reorder-native-value="2"][data-reorder-chip="1"]'),
    );
    assert.ok(
      first.textContent!.includes('[data-reorder-native-value="2"][data-reorder-chip="2"]'),
    );
    assert.equal(
      owner.prepare([0], 'held', () => true),
      true,
    );
    assert.equal(sheet(), first);
    assert.ok(
      first.textContent!.includes('[data-reorder-native-value="2"][data-reorder-chip="0"]'),
    );
    assert.equal(
      first.textContent!.includes('[data-reorder-native-value="2"][data-reorder-chip="1"]'),
      false,
    );
    assert.equal(
      first.textContent!.includes('[data-reorder-native-value="2"][data-reorder-chip="2"]'),
      false,
    );
    f.view.dom.setAttribute('data-reorder-native-value', '3');
    assert.equal(
      owner.prepare([0], 'held', () => true),
      true,
    );
    assert.equal(sheet(), first);
    assert.ok(
      first.textContent!.includes('[data-reorder-native-value="3"][data-reorder-chip="0"]'),
    );
    assert.equal(first.textContent!.includes('[data-reorder-native-value="2"]'), false);
    for (const chip of f.chips) assert.equal(chip.getAttribute('data-reorder-motion-local'), null);
  } catch (cause) {
    causes.push(cause);
  }
  try {
    owner.retire();
  } catch (cause) {
    causes.push(cause);
  }
  if (causes.length === 1) throw causes[0];
  if (causes.length) throw new AggregateError(causes, 'native-mask-owner-retirement-failed');
});
