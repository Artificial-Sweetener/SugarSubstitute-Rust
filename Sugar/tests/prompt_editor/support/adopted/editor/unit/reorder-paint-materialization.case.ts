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
/** Request-local prototypes reuse exact native style output, with isolated copies and explicit finite fallback. */
import assert from 'node:assert/strict';
import test from 'node:test';
import {
  ReorderPaintMaterialization,
  withReorderPaintMaterialization,
} from '../../../../../../frontend/sugarprompt/editor/src/reorder-paint-materialization.ts';
import { copyNativeReorderPaint } from '../../../../../../frontend/sugarprompt/editor/src/reorder-native-paint.ts';
import type { EditorView } from '@codemirror/view';
import type { PreparedReorderPaintNode } from '../../../../../../frontend/sugarprompt/editor/src/reorder-paint-preparation.ts';
import {
  platform,
  descriptor,
  safety,
  type PlatformNode,
} from './reorder-paint-materialization-fixture.ts';
test('exact prepared style output is reused across distinct roots without sharing copy mutations', () => {
  const p = platform(),
    request = new ReorderPaintMaterialization(p.document as unknown as Document);
  const first = {} as Node,
    second = {} as Node,
    copies = new WeakMap<Node, Node>(),
    css = descriptor();
  const prepared = new Map<Node, PreparedReorderPaintNode>([
    [
      first,
      {
        ...css,
        className: 'first',
        attributes: [['data-resolution', 'found']],
        readonly: true,
      },
    ],
    [
      second,
      {
        ...css,
        className: 'second',
        attributes: [['data-resolution', 'missing']],
      },
    ],
  ]);
  try {
    const a = copyNativeReorderPaint(
      { dom: { ownerDocument: p.document } } as unknown as EditorView,
      first,
      prepared,
      request,
      copies,
    ) as unknown as (typeof p.nodes)[number];
    assert.ok(a);
    safety(a);
    assert.equal(a.style.cssText, css.cssText);
    assert.equal(a.contentEditable, 'false');
    a.style.cssText = 'color:red;';
    a.style.setProperty('pointer-events', 'auto');
    a.attributes.set('data-resolution', 'changed');
    a.children.push('changed');
    const b = copyNativeReorderPaint(
      { dom: { ownerDocument: p.document } } as unknown as EditorView,
      second,
      prepared,
      request,
      copies,
    ) as unknown as (typeof p.nodes)[number];
    assert.ok(b);
    safety(b);
    assert.equal(b.style.cssText, css.cssText);
    assert.equal(b.className, 'second');
    assert.equal(b.attributes.get('data-resolution'), 'missing');
    assert.equal(b.contentEditable, 'inherit');
    assert.deepEqual(b.children, []);
    assert.notEqual(a, b);
    assert.equal(copies.get(first), a);
    assert.equal(copies.get(second), b);
    assert.equal(p.publications, 2); // One template publication plus the deliberately mutated first output.
    assert.equal(p.clones, 2);
    assert.equal(p.allocations, 3);
  } finally {
    request.dispose();
  }
  assert.throws(() => request.create(css), /disposed/u);
});

test('exact style/tag/namespace keys are independent and a new request owns fresh prototypes', () => {
  const p = platform(),
    r = new ReorderPaintMaterialization(p.document as unknown as Document);
  try {
    for (const d of [
      descriptor(),
      descriptor('color:blue;'),
      descriptor(undefined, 'wbr'),
      descriptor(undefined, 'svg', true),
    ]) {
      const a = r.create(d) as unknown as (typeof p.nodes)[number];
      assert.equal(a.style.cssText, d.cssText);
      assert.equal(a.localName, d.tag);
      assert.equal(
        a.namespaceURI,
        d.vector ? 'http://www.w3.org/2000/svg' : 'http://www.w3.org/1999/xhtml',
      );
      safety(a);
    }
    assert.equal(p.publications, 4);
  } finally {
    r.dispose();
  }
  const next = new ReorderPaintMaterialization(p.document as unknown as Document);
  try {
    next.create(descriptor());
    assert.equal(p.publications, 5);
  } finally {
    next.dispose();
  }
});

test('64 templates bound extra allocations while every overflow output retains complete direct style', () => {
  const p = platform(),
    r = new ReorderPaintMaterialization(p.document as unknown as Document);
  try {
    for (let i = 0; i < 1024; ++i) {
      const d = descriptor(`color:rgb(${i % 255}, 0, 0);--literal-index:${i};`),
        n = r.create(d) as unknown as (typeof p.nodes)[number];
      assert.equal(n.style.cssText, d.cssText);
      safety(n);
    }
    assert.equal(p.allocations, 1088);
    assert.equal(p.clones, 64);
    assert.equal(p.publications, 1024);
    const before = p.publications,
      n = r.create(
        descriptor('color:rgb(0, 0, 0);--literal-index:0;'),
      ) as unknown as (typeof p.nodes)[number];
    assert.equal(p.publications, before);
    assert.equal(n.style.cssText, 'color:rgb(0, 0, 0);--literal-index:0;');
    safety(n);
  } finally {
    r.dispose();
  }
});

test('canvas, decoded media and widget buffers never allocate nonpixel prototypes', () => {
  const p = platform(),
    r = new ReorderPaintMaterialization(p.document as unknown as Document);
  try {
    for (const d of [
      { ...descriptor(undefined, 'canvas') },
      { ...descriptor(), buffer: true },
      {
        ...descriptor(),
        media: { source: {} as HTMLCanvasElement, width: 2, height: 3 },
      },
    ]) {
      const n = r.create(d) as unknown as (typeof p.nodes)[number];
      assert.equal(n.style.cssText, d.cssText);
      safety(n);
    }
    assert.equal(p.allocations, 3);
    assert.equal(p.clones, 0);
    assert.equal(p.publications, 3);
  } finally {
    r.dispose();
  }
});

for (const stage of ['allocate', 'style', 'clone'] as const)
  for (const cause of [null, undefined, new Error('controlled-native-prototype-failure')])
    test(`raw prototype ${stage} failure preserves ${String(cause)} and retirement rejects future use`, () => {
      const p = platform({ stage, cause }),
        r = new ReorderPaintMaterialization(p.document as unknown as Document);
      let failed = false,
        actual: unknown;
      try {
        r.create(descriptor());
      } catch (error) {
        failed = true;
        actual = error;
      } finally {
        r.dispose();
        r.dispose();
      }
      assert.equal(failed, true);
      assert.equal(actual, cause);
      assert.throws(() => r.create(descriptor()), /disposed/u);
    });

for (const primary of [null, undefined, new Error('controlled-prototype-action')]) {
  test(`materialization retains raw ${String(primary)} before all attempted template cleanup failures`, () => {
    const p = platform(),
      cleanup = [null, undefined],
      attempts: number[] = [];
    let failed = false,
      actual: unknown;
    try {
      withReorderPaintMaterialization(
        p.document as unknown as Document,
        (r) => {
          r.create(descriptor());
          r.create(descriptor('color:blue;'));
          for (const [at, index] of [0, 2].entries())
            p.nodes[index]!.remove = () => {
              attempts.push(index);
              // eslint-disable-next-line @typescript-eslint/only-throw-error -- Inject raw host failures to prove original cause retention and cleanup.
              throw cleanup[at];
            };
          // eslint-disable-next-line @typescript-eslint/only-throw-error -- Inject raw host failures to prove original cause retention and cleanup.
          throw primary;
        },
        () => {},
      );
    } catch (cause) {
      failed = true;
      actual = cause;
    }
    assert.equal(failed, true);
    assert.ok(actual instanceof AggregateError);
    assert.deepEqual(actual.errors, [primary, ...cleanup]);
    assert.deepEqual(attempts, [0, 2]);
  });
}
test('successful materialization cannot hide all attempted prototype cleanup errors', () => {
  const p = platform(),
    first = new Error('controlled-first-template-release'),
    second = new Error('controlled-second-template-release');
  assert.throws(
    () =>
      withReorderPaintMaterialization(
        p.document as unknown as Document,
        (r) => {
          r.create(descriptor());
          r.create(descriptor('color:red;'));
          p.nodes[0]!.remove = () => {
            throw first;
          };
          p.nodes[2]!.remove = () => {
            throw second;
          };
          return 'accepted';
        },
        () => {},
      ),
    (cause) =>
      cause instanceof AggregateError && cause.errors[0] === first && cause.errors[1] === second,
  );
});
test('partially styled template is retired after raw style failure', () => {
  const cause = new Error('controlled-partial-template-style'),
    p = platform({ stage: 'style', cause });
  assert.throws(
    () =>
      withReorderPaintMaterialization(
        p.document as unknown as Document,
        (r) => r.create(descriptor()),
        (node) => node.remove(),
      ),
    (error) => error === cause,
  );
  assert.equal(p.allocations, 1);
  assert.equal(p.removals, 1);
  assert.equal(p.clones, 0);
});
test('retirement reentry cannot revive old request or retire a distinct new materialization', () => {
  const p = platform(),
    old = new ReorderPaintMaterialization(p.document as unknown as Document),
    next = new ReorderPaintMaterialization(p.document as unknown as Document);
  old.create(descriptor());
  let output: HTMLElement | null = null;
  p.nodes[0]!.remove = () => {
    assert.throws(() => old.create(descriptor()), /disposed/u);
    output = next.create(descriptor());
  };
  old.dispose();
  assert.ok(output);
  assert.equal((output as unknown as PlatformNode).style.cssText, descriptor().cssText);
  old.dispose();
  assert.equal(p.removals, 0);
  next.dispose();
  assert.equal(p.removals, 1);
});

test('caught partial style failures still consume the64-template ownership cap', () => {
  const cause = new Error('controlled-partial-style'),
    p = platform({ stage: 'style', cause }),
    r = new ReorderPaintMaterialization(p.document as unknown as Document);
  for (let i = 0; i < 65; ++i)
    assert.throws(
      () => r.create(descriptor(`color:rgb(${i},0,0);`)),
      (error) => error === cause,
    );
  assert.equal(p.allocations, 65);
  r.dispose();
  assert.equal(p.removals, 64);
  assert.equal(p.clones, 0);
});
for (const phase of ['construction', 'setter', 'cached-clone'] as const)
  test(`native ${phase} retirement refuses publication and removes untransferred identity`, () => {
    const p = platform(),
      r = new ReorderPaintMaterialization(p.document as unknown as Document),
      create = p.document.createElement.bind(p.document);
    if (phase === 'cached-clone') {
      r.create(descriptor());
      const template = p.nodes[0]!,
        clone = template.cloneNode.bind(template);
      template.cloneNode = (deep) => {
        const copy = clone(deep);
        r.dispose();
        return copy;
      };
    } else
      p.document.createElement = (tag) => {
        const node = create(tag);
        if (phase === 'construction') r.dispose();
        else {
          const set = node.style.setProperty.bind(node.style);
          node.style.setProperty = (...args) => {
            set(...args);
            r.dispose();
          };
        }
        return node;
      };
    assert.throws(() => r.create(descriptor()), /retired/u);
    assert.throws(() => r.create(descriptor()), /disposed/u);
    r.dispose();
    assert.equal(p.removals, phase === 'cached-clone' ? 2 : 1);
  });
test('retired cached clone keeps native retirement primary before its removal failure', () => {
  const p = platform(),
    r = new ReorderPaintMaterialization(p.document as unknown as Document);
  r.create(descriptor());
  const template = p.nodes[0]!,
    clone = template.cloneNode.bind(template),
    cleanup = new Error('controlled-untransferred-clone-removal');
  template.cloneNode = (deep) => {
    const copy = clone(deep);
    r.dispose();
    copy.remove = () => {
      throw cleanup;
    };
    return copy;
  };
  assert.throws(
    () => r.create(descriptor()),
    (cause) =>
      cause instanceof AggregateError &&
      cause.errors[0] instanceof Error &&
      cause.errors[0].message === 'reorder-paint-materialization-retired' &&
      cause.errors[1] === cleanup,
  );
  r.dispose();
});
