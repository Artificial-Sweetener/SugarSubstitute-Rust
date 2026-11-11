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
/** Independent native-boundary doubles prove finite row-edge leases; browser proof owns CSS line-box correctness. */
import assert from 'node:assert/strict';
import test from 'node:test';
import type { EditorView } from '@codemirror/view';
import {
  mountCanonicalRowEdges,
  readCanonicalRowEdges,
  releaseCanonicalRowEdges,
} from '../../../../../../frontend/sugarprompt/editor/src/canonical-row-edges.ts';
/** Minimal exact native element boundary; literal rectangles and failure hooks are independent oracle operands. */
class Element {
  className = '';
  readonly created: Element[];
  readonly ownerDocument: { createElement: () => HTMLElement };
  line: Element | null = null;
  left = 72;
  top = 10;
  bottom = 29;
  reads = 0;
  removed = 0;
  removeHook: (() => void) | null = null;
  prependHook: (() => void) | null = null;
  appendHook: (() => void) | null = null;
  parent: Element | null = null;
  readonly children: Element[] = [];
  /** Every create call produces a distinct controlled element, without a browser or hidden editor. */
  constructor(created: Element[]) {
    this.created = created;
    this.ownerDocument = {
      createElement: () => {
        const node = new Element(created);
        created.push(node);
        return node.native();
      },
    };
  }
  /** This exact explicit external fixture boundary implements only the native methods consumed by the owner. */
  native(): HTMLElement {
    return this as unknown as HTMLElement;
  }
  /** Literal attributes introduce no fixture semantics. */
  setAttribute(): void {}
  /** The controlled reentrant boundary precedes the native backing write, exposing late insertion after cancellation. */
  prepend(node: Element): void {
    this.prependHook?.();
    this.children.unshift(node);
    node.parent = this;
  }
  /** Append retains literal node order for independent cleanup assertions. */
  append(node: Element): void {
    this.appendHook?.();
    this.children.push(node);
    node.parent = this;
  }
  /** The controlled native ancestor belongs to this original row; no layout authority is synthesized. */
  closest(): HTMLElement | null {
    return this.line?.native() ?? null;
  }
  /** Independent reads expose only fixed closed geometry. */
  getBoundingClientRect(): Pick<DOMRect, 'top' | 'bottom' | 'left'> {
    this.reads++;
    return { top: this.top, bottom: this.bottom, left: this.left };
  }
  /** Raw failure hooks deliberately include null and undefined; every removal remains separately observable. */
  remove(): void {
    this.removed++;
    this.removeHook?.();
    if (this.parent) {
      const at = this.parent.children.indexOf(this);
      if (at >= 0) this.parent.children.splice(at, 1);
      this.parent = null;
    }
  }
}
/** Current native membership excludes passive roots without creating another editable state. */
function fixture() {
  const created: Element[] = [],
    root = new Element(created),
    connected = new Set<HTMLElement>([root.native()]);
  const membership = {
    read: (node: Node) => connected.has(node as HTMLElement),
  };
  const view = {
    contentDOM: { contains: (node: Node) => membership.read(node) },
  } as unknown as EditorView;
  return { root, view, created, connected, membership };
}
test('overlapping live rows merge once while passive roots remain excluded and every lease retires once', () => {
  const f = fixture(),
    peer = new Element(f.created),
    passive = new Element(f.created);
  f.connected.add(peer.native());
  try {
    assert.equal(
      mountCanonicalRowEdges(
        f.view,
        f.root.native(),
        { sceneFill: true, region: null },
        () => true,
      ),
      true,
    );
    assert.equal(
      mountCanonicalRowEdges(f.view, peer.native(), { sceneFill: true, region: null }, () => true),
      true,
    );
    assert.equal(
      mountCanonicalRowEdges(
        f.view,
        passive.native(),
        { sceneFill: true, region: null },
        () => true,
      ),
      true,
    );
    assert.deepEqual(
      readCanonicalRowEdges(f.view, () => true),
      [
        {
          top: 10,
          bottom: 29,
          left: null,
          paint: { sceneFill: true, region: null },
        },
      ],
    );
    assert.equal(
      f.created.slice(4).reduce((sum, node) => sum + node.reads, 0),
      0,
    );
  } finally {
    for (const root of [f.root, peer, passive]) releaseCanonicalRowEdges(root.native());
  }
  for (const node of f.created) assert.equal(node.removed, 1);
  releaseCanonicalRowEdges(f.root.native());
  assert.deepEqual(
    readCanonicalRowEdges(f.view, () => true),
    [],
  );
});
test('retirement during membership prevents a read from the obsolete native edge', () => {
  const f = fixture();
  assert.equal(
    mountCanonicalRowEdges(f.view, f.root.native(), { sceneFill: true, region: null }, () => true),
    true,
  );
  f.membership.read = () => {
    releaseCanonicalRowEdges(f.root.native());
    return true;
  };
  assert.equal(
    readCanonicalRowEdges(f.view, () => true),
    null,
  );
  assert.equal(
    f.created.reduce((sum, node) => sum + node.reads, 0),
    0,
  );
});
test('partial acquisition preserves a raw null primary and undefined cleanup while attempting both edges', () => {
  const f = fixture();
  f.root.prependHook = () => {
    f.created[0]!.removeHook = () => {
      // eslint-disable-next-line @typescript-eslint/only-throw-error -- Inject raw host failures to prove original cause retention and cleanup.
      throw undefined;
    };
    // eslint-disable-next-line @typescript-eslint/only-throw-error -- Inject raw host failures to prove original cause retention and cleanup.
    throw null;
  };
  assert.throws(
    () =>
      mountCanonicalRowEdges(
        f.view,
        f.root.native(),
        { sceneFill: true, region: null },
        () => true,
      ),
    (error: unknown) =>
      error instanceof AggregateError &&
      error.errors.length === 2 &&
      error.errors[0] === null &&
      error.errors[1] === undefined,
  );
  assert.deepEqual(
    f.created.map((node) => node.removed),
    [1, 1],
  );
  assert.deepEqual(
    readCanonicalRowEdges(f.view, () => true),
    [],
  );
});
for (const boundary of ['prependHook', 'appendHook'] as const)
  test(`a cancelled ${boundary} cannot leave late-written edges or retire its replacement`, () => {
    const f = fixture();
    let old = true;
    f.root[boundary] = () => {
      f.root[boundary] = null;
      old = false;
      releaseCanonicalRowEdges(f.root.native());
      assert.equal(
        mountCanonicalRowEdges(
          f.view,
          f.root.native(),
          { sceneFill: true, region: null },
          () => true,
        ),
        true,
      );
    };
    assert.equal(
      mountCanonicalRowEdges(f.view, f.root.native(), { sceneFill: true, region: null }, () => old),
      false,
    );
    assert.deepEqual(f.root.children, f.created.slice(2));
    assert.equal(f.created[0]!.parent, null);
    assert.equal(f.created[1]!.parent, null);
    assert.deepEqual(
      readCanonicalRowEdges(f.view, () => true),
      [
        {
          top: 10,
          bottom: 29,
          left: null,
          paint: { sceneFill: true, region: null },
        },
      ],
    );
    releaseCanonicalRowEdges(f.root.native());
    assert.deepEqual(f.root.children, []);
    assert.deepEqual(
      f.created.slice(2).map((node) => node.removed),
      [1, 1],
    );
  });

/** Each new regional proof preserves its body primary before attempting every acquired row lease retirement. */
function regionalProof(body: (f: ReturnType<typeof fixture>, owned: Element[]) => void): void {
  const f = fixture(),
    owned = [f.root],
    causes: unknown[] = [];
  try {
    body(f, owned);
  } catch (cause) {
    causes.push(cause);
  }
  for (const root of owned)
    try {
      releaseCanonicalRowEdges(root.native());
    } catch (cause) {
      causes.push(cause);
    }
  if (causes.length === 1) throw causes[0];
  if (causes.length) throw new AggregateError(causes, 'regional-row-proof-retirement');
}

test('regional-only rows use one actual native axis read while distinct touching rows retain full-height ownership', () =>
  regionalProof((f, owned) => {
    const peer = new Element(f.created),
      passive = new Element(f.created),
      line = new Element([]),
      region = {
        scopeId: 'scope',
        ordinal: 0,
        normal: 'rgb(79,109,245)',
        hover: 'rgb(158,176,255)',
      },
      paint = { sceneFill: false, region };
    owned.push(peer, passive);
    f.root.line = line;
    peer.line = line;
    f.connected.add(peer.native());
    assert.equal(
      mountCanonicalRowEdges(f.view, f.root.native(), paint, () => true),
      true,
    );
    assert.equal(
      mountCanonicalRowEdges(f.view, peer.native(), paint, () => true),
      true,
    );
    assert.equal(
      mountCanonicalRowEdges(f.view, passive.native(), paint, () => true),
      true,
    );
    f.created[2]!.top = 29;
    f.created[3]!.bottom = 48;
    assert.deepEqual(
      readCanonicalRowEdges(f.view, () => true),
      [
        { top: 10, bottom: 29, left: 72, paint },
        { top: 29, bottom: 48, left: 72, paint },
      ],
    );
    assert.equal(line.reads, 1);
    assert.equal(f.created[4]!.reads + f.created[5]!.reads, 0);
  }));

test('retirement at the native axis frontier rejects captured regional output before publication', () =>
  regionalProof((f) => {
    const line = new Element([]),
      paint = {
        sceneFill: false,
        region: {
          scopeId: 'scope',
          ordinal: 0,
          normal: 'rgb(79,109,245)',
          hover: 'rgb(158,176,255)',
        },
      };
    f.root.line = line;
    assert.equal(
      mountCanonicalRowEdges(f.view, f.root.native(), paint, () => true),
      true,
    );
    let current = true;
    line.getBoundingClientRect = () => {
      current = false;
      return { top: 10, bottom: 29, left: 72 };
    };
    assert.equal(
      readCanonicalRowEdges(f.view, () => current),
      null,
    );
  }));
