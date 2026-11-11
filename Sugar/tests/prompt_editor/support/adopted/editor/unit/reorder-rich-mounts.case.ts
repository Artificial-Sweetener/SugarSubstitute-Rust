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
/** Actual RangeSet point iteration owns display-only mounts; a controlled DOM transport does not certify browser paint. */
import assert from 'node:assert/strict';
import test from 'node:test';
import { EditorView, Decoration } from '@codemirror/view';
import {
  SourceToken,
  SourceWidget,
} from '../../../../../../frontend/sugarprompt/editor/src/projection-widgets.ts';
import {
  renderReorderFragment,
  releaseReorderFragment,
} from '../../../../../../frontend/sugarprompt/editor/src/reorder-projection.ts';
import type { SourceRange } from '../../../../../../frontend/sugarprompt/core/src/index.ts';

/** These plain-source characterizations observe literal spans in their declared decoded coordinate space. */
const literal =
  (text: string) =>
  (range: SourceRange): string =>
    text.slice(range.from, range.to);

/** Minimal literal DOM boundary observes appended nodes and excludes a simulated editing/rendering engine. */
class DisplayNode {
  readonly localName: string;
  /** Preserve actual element kind so zero-text wrapping hints are observable independently of widgets. */
  constructor(localName = 'span') {
    this.localName = localName;
  }
  readonly children: DisplayNode[] = [];
  textContent = '';
  className = '';
  contentEditable = 'inherit';
  readonly attributes = new Map<string, string>();
  /** Retain only literal attribute intent in this transport. */
  setAttribute(name: string, value: string): void {
    this.attributes.set(name, value);
  }
  /** Observe actual mount order without layout authority. */
  append(...nodes: DisplayNode[]): void {
    this.children.push(...nodes);
  }
}
/** The sole cast adapts a controlled external DOM boundary to supported WidgetType methods. */
function domTransport() {
  const document = {
    createElement(name: string): HTMLElement {
      return new DisplayNode(name) as unknown as HTMLElement;
    },
    createTextNode(text: string): Node {
      const node = new DisplayNode('#text');
      node.textContent = text;
      return node as unknown as Node;
    },
  };
  const view = Object.create(EditorView.prototype) as EditorView;
  Object.defineProperty(view, 'dom', { value: { ownerDocument: document } });
  return { view, root: (): HTMLElement => document.createElement('span') };
}

test('one continuing canonical mark owns adjacent literal and readonly widget descendants', () => {
  const f = domTransport(),
    first = new ObservedWidget(),
    second = new ObservedWidget(),
    root = f.root(),
    decorations = Decoration.set(
      [
        Decoration.mark({ class: 'outer', inclusive: true }).range(0, 5),
        Decoration.mark({ class: 'inner', inclusive: true }).range(1, 4),
        Decoration.replace({ widget: first }).range(1, 2),
        Decoration.replace({ widget: second }).range(3, 4),
      ],
      true,
    );
  renderReorderFragment(f.view, root, literal('abcde'), { from: 0, to: 5 }, decorations, null);
  const children = (root as unknown as DisplayNode).children;
  assert.equal(children.length, 1);
  assert.equal(children[0]!.className, 'outer');
  assert.equal(children[0]!.children.length, 3);
  assert.equal(children[0]!.children[0]!.textContent, 'a');
  assert.equal(children[0]!.children[2]!.textContent, 'e');
  const inner = children[0]!.children[1]!;
  assert.equal(inner.className, 'inner');
  assert.deepEqual(
    inner.children.map((node) => node.localName),
    ['wbr', 'span', 'wbr', '#text', 'wbr', 'span', 'wbr'],
  );
  assert.equal(inner.children[1]!.contentEditable, 'false');
  assert.equal(inner.children[3]!.textContent, 'c');
  assert.equal(inner.children[5]!.contentEditable, 'false');
  for (const at of [0, 2, 4, 6]) assert.equal(inner.children[at]!.textContent, '');
  assert.deepEqual([first.mounted, second.mounted], [1, 1]);
  releaseReorderFragment(root);
  assert.deepEqual([first.released, second.released], [1, 1]);
});

test('equal closed and reopened canonical marks retain distinct ancestors', () => {
  const f = domTransport(),
    root = f.root(),
    first = Decoration.mark({ class: 'scope' }),
    second = Decoration.mark({ class: 'scope' });
  assert.equal(first.eq(second), true);
  renderReorderFragment(
    f.view,
    root,
    literal('abcd'),
    { from: 0, to: 4 },
    Decoration.set([first.range(0, 2), second.range(2, 4)]),
    null,
  );
  const children = (root as unknown as DisplayNode).children;
  assert.equal(children.length, 2);
  assert.notEqual(children[0], children[1]);
  assert.equal(children[0]!.children[0]!.textContent, 'ab');
  assert.equal(children[1]!.children[0]!.textContent, 'cd');
  releaseReorderFragment(root);
});
/** A genuine shared widget reports only actual mount/destruction counts and controlled boundary failures. */
class ObservedWidget extends SourceWidget {
  mounted = 0;
  released = 0;
  readonly mountFailure: Error | null;
  readonly releaseFailure: Error | null;
  /** Bind finite display observation without another projection owner. */
  constructor(mountFailure: Error | null = null, releaseFailure: Error | null = null) {
    super(new SourceToken());
    this.mountFailure = mountFailure;
    this.releaseFailure = releaseFailure;
  }
  /** The actual renderer must call the shared widget exactly once for a full replacement. */
  override toDOM(view: EditorView): HTMLElement {
    if (this.mountFailure) throw this.mountFailure;
    this.mounted++;
    return view.dom.ownerDocument.createElement('span');
  }
  /** Record one owned release and preserve controlled unexpected causes. */
  override destroy(_dom: HTMLElement): void {
    this.released++;
    if (this.releaseFailure) throw this.releaseFailure;
  }
}

test("empty carriers and ending boundaries cannot mount the adjacent chip's replacement", () => {
  const f = domTransport(),
    first = new ObservedWidget(),
    next = new ObservedWidget();
  const decorations = Decoration.set([
    Decoration.replace({ widget: first }).range(0, 1),
    Decoration.replace({ widget: next }).range(2, 4),
  ]);
  for (const at of [0, 2])
    renderReorderFragment(
      f.view,
      f.root(),
      literal('abcd'),
      { from: at, to: at },
      decorations,
      null,
    );
  assert.deepEqual([first.mounted, next.mounted], [0, 0]);
  const left = f.root(),
    right = f.root();
  renderReorderFragment(f.view, left, literal('abcd'), { from: 0, to: 2 }, decorations, null);
  assert.deepEqual([first.mounted, next.mounted], [1, 0]);
  renderReorderFragment(f.view, right, literal('abcd'), { from: 2, to: 4 }, decorations, null);
  assert.deepEqual([first.mounted, next.mounted], [1, 1]);
  releaseReorderFragment(left);
  releaseReorderFragment(right);
  releaseReorderFragment(right);
  assert.deepEqual([first.released, next.released], [1, 1]);
});

test('a fragment cutting a replacement refuses partial widget paint instead of duplicating it', () => {
  const f = domTransport(),
    widget = new ObservedWidget();
  const root = f.root(),
    decorations = Decoration.set([Decoration.replace({ widget }).range(0, 3)]);
  assert.throws(
    () =>
      renderReorderFragment(f.view, root, literal('abc'), { from: 1, to: 3 }, decorations, null),
    /reorder-projection-fragment-split-widget/u,
  );
  assert.deepEqual([widget.mounted, widget.released], [0, 0]);
  releaseReorderFragment(root);
});

test('canonical inclusive regional marks wrap the complete shared replacement at the same boundaries', () => {
  const f = domTransport(),
    widget = new ObservedWidget(),
    root = f.root();
  const decorations = Decoration.set(
    [
      Decoration.mark({
        attributes: {
          class: 'sp-region-content',
          style: 'color: rgb(20, 30, 40)',
        },
        inclusive: true,
      }).range(0, 3),
      Decoration.replace({ widget }).range(0, 3),
    ],
    true,
  );
  renderReorderFragment(f.view, root, literal('abc'), { from: 0, to: 3 }, decorations, null);
  const nodes = (root as unknown as DisplayNode).children;
  assert.equal(nodes.length, 1);
  assert.equal(nodes[0]!.attributes.get('class'), 'sp-region-content');
  assert.deepEqual(
    nodes[0]!.children.map((node) => node.localName),
    ['wbr', 'span', 'wbr'],
  );
  assert.equal(widget.mounted, 1);
  releaseReorderFragment(root);
  assert.equal(widget.released, 1);
});

test('failed later paint releases earlier shared mounts and preserves ordered original/cleanup causes', () => {
  const f = domTransport(),
    cause = new Error('controlled-paint-failure'),
    cleanup = new Error('controlled-paint-release-failure');
  const first = new ObservedWidget(null, cleanup),
    second = new ObservedWidget(cause);
  const root = f.root(),
    decorations = Decoration.set([
      Decoration.replace({ widget: first }).range(0, 1),
      Decoration.replace({ widget: second }).range(1, 2),
    ]);
  assert.throws(
    () => renderReorderFragment(f.view, root, literal('ab'), { from: 0, to: 2 }, decorations, null),
    (error: unknown) => {
      assert.ok(error instanceof AggregateError);
      assert.deepEqual(error.errors, [cause, cleanup]);
      return true;
    },
  );
  assert.deepEqual([first.mounted, first.released, second.mounted], [1, 1, 0]);
  releaseReorderFragment(root);
  assert.equal(first.released, 1);
});

/** This exact controlled native DOM setter fails after a complete shared widget is already constructed. */
class ReadonlyFailureWidget extends ObservedWidget {
  readonly failure: Error;
  /** Retain independent write and resource-retirement failures. */
  constructor(failure: Error, cleanup: Error | null) {
    super(null, cleanup);
    this.failure = failure;
  }
  /** Preserve the genuine shared mount, then control only its owned noneditable DOM declaration. */
  override toDOM(view: EditorView): HTMLElement {
    const dom = super.toDOM(view),
      failure = this.failure;
    Object.defineProperty(dom, 'contentEditable', {
      get: () => 'inherit',
      set: (_value: string) => {
        throw failure;
      },
    });
    return dom;
  }
}

test('shared replacement roots explicitly own readonly boundaries while ordinary literal text inherits', () => {
  const f = domTransport(),
    widget = new ObservedWidget(),
    root = f.root();
  renderReorderFragment(
    f.view,
    root,
    literal('ab'),
    { from: 0, to: 2 },
    Decoration.set([Decoration.replace({ widget }).range(0, 1)]),
    null,
  );
  const nodes = (root as unknown as DisplayNode).children;
  assert.deepEqual(
    nodes.map((node) => node.localName),
    ['wbr', 'span', 'wbr', '#text'],
  );
  assert.equal(nodes[1]!.contentEditable, 'false');
  assert.equal(nodes[3]!.contentEditable, 'inherit');
  assert.deepEqual(
    nodes.map((node) => node.textContent),
    ['', '', '', 'b'],
  );
  assert.deepEqual([widget.mounted, widget.released], [1, 0]);
  releaseReorderFragment(root);
  assert.equal(widget.released, 1);
});

for (const failsCleanup of [false, true])
  test(`readonly declaration failure retires the registered shared mount exactly once, cleanup=${failsCleanup}`, () => {
    const f = domTransport(),
      cause = new Error('controlled-readonly-declaration'),
      cleanup = failsCleanup ? new Error('controlled-readonly-retirement') : null,
      widget = new ReadonlyFailureWidget(cause, cleanup),
      root = f.root();
    assert.throws(
      () =>
        renderReorderFragment(
          f.view,
          root,
          literal('a'),
          { from: 0, to: 1 },
          Decoration.set([Decoration.replace({ widget }).range(0, 1)]),
          null,
        ),
      (error: unknown) => {
        if (cleanup) {
          assert.ok(error instanceof AggregateError);
          assert.deepEqual(error.errors, [cause, cleanup]);
        } else assert.equal(error, cause);
        return true;
      },
    );
    assert.equal((root as unknown as DisplayNode).children.length, 0);
    assert.deepEqual([widget.mounted, widget.released], [1, 1]);
    releaseReorderFragment(root);
    assert.equal(widget.released, 1);
  });
