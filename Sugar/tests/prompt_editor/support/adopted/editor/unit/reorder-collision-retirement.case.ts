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
/** Actual collision resources retire detached, preserving finite authority and ordered cleanup failures. */
import assert from 'node:assert/strict';
import { sceneErrors } from '../../../../../../frontend/sugarprompt/editor/src/scene-errors.ts';
import test from 'node:test';
import { undoDepth } from '@codemirror/commands';
import { Decoration, type EditorView } from '@codemirror/view';
import { hideReorderChip } from '../../../../../../frontend/sugarprompt/core/src/index.ts';
import {
  SourceToken,
  SourceWidget,
} from '../../../../../../frontend/sugarprompt/editor/src/projection-widgets.ts';
import type { ReadonlyWidgetMedia } from '../../../../../../frontend/sugarprompt/editor/src/readonly-widget-media.ts';
import { ReorderCollisionBase } from '../../../../../../frontend/sugarprompt/editor/src/reorder-collision-base.ts';
import { renderReorderFragment } from '../../../../../../frontend/sugarprompt/editor/src/reorder-projection.ts';
import { reorderPreview } from '../../../../../../frontend/sugarprompt/editor/src/reorder-preview.ts';
import { fixture, key } from './reorder-session-fixture.ts';

/** A real canonical widget exposes its supplied media lifetime and native child retirement. */
class RetirementPaint extends SourceWidget {
  media: ReadonlyWidgetMedia | null = null;
  mounted = 0;
  released = 0;
  /** The actual canonical lifetime owns this callback independently of native DOM attachment. */
  private readonly release: (node: HTMLElement) => void;
  /** Each independently mounted canonical fragment owns its release callback. */
  constructor(release: (node: HTMLElement) => void) {
    super(new SourceToken());
    this.release = release;
  }
  /** Literal native output is produced through the existing canonical renderer. */
  override toDOM(view: EditorView): HTMLElement {
    this.mounted++;
    const node = view.dom.ownerDocument.createElement('span');
    node.textContent = 'b';
    return node;
  }
  /** Observe the actual collision media instance rather than replacing its owner. */
  override toReadonlyDOM(view: EditorView, media: ReadonlyWidgetMedia): HTMLElement {
    this.media = media;
    return this.toDOM(view);
  }
  /** The canonical fragment owner invokes this exactly once for each mounted output. */
  override destroy(node: HTMLElement): void {
    this.released++;
    this.release(node);
  }
}

/** Mount real collision paint; only font events and resource callbacks are controlled external boundaries. */
function retirementFixture() {
  const f = fixture(),
    base = new ReorderCollisionBase(f.view),
    fonts = new EventTarget();
  let fontRemoves = 0;
  const removeFont = fonts.removeEventListener.bind(fonts);
  fonts.removeEventListener = (...args) => {
    fontRemoves++;
    removeFont(...args);
  };
  let observe: (role: number, node: HTMLElement) => void = () => {};
  const paints = [0, 1, 2].map((role) => new RetirementPaint((node) => observe(role, node)));
  try {
    Object.defineProperty(f.view.dom.ownerDocument, 'fonts', { value: fonts });
    f.owner.keydown(key('Alt'));
    f.measure();
    f.task();
    const admitted = f.state.field(reorderPreview).value!,
      range = admitted.output.renderedRanges[1]!;
    const canonical = Decoration.set([
      Decoration.replace({ widget: paints[0]! }).range(range.from, range.to),
    ]);
    const capture = {
      ...admitted.capture,
      projection: {
        canonical,
        visible: canonical,
        decoded: canonical,
        ordinary: canonical,
        sceneErrors: f.state.field(sceneErrors),
      },
    };
    const hidden = hideReorderChip(capture.prepared, admitted.order, admitted.activeChip)!;
    const input = {
      token: admitted.token,
      capture,
      order: admitted.order,
      hidden,
      held: admitted.activeChip,
      pointer: 9,
      coordinates: { x: 55, y: 20 },
    };
    assert.equal(base.prepare(input), true);
    const context = base.contextAt(input)!;
    base.mount(input, context);
    const root = Array.from(f.view.dom.ownerDocument.body.childNodes).find((node) =>
      (node as HTMLElement).className.includes('sp-reorder-base-measurement'),
    ) as HTMLElement;
    assert.equal(!!root && root.isConnected, true);
    const line = root.childNodes[0]!.childNodes[0]!.childNodes[0]!;
    // Register independent actual fragment leases on the two owned external edge containers.
    for (const [at, edge] of [line.childNodes[0]!, line.childNodes[2]!].entries()) {
      renderReorderFragment(
        f.view,
        edge as HTMLElement,
        () => 'a',
        { from: 0, to: 1 },
        Decoration.set([Decoration.replace({ widget: paints[at + 1]! }).range(0, 1)]),
        null,
      );
    }
    const media = paints[0]!.media!;
    assert.equal(media !== null && paints.every((paint) => paint.mounted === 1), true);
    const source = f.state.doc,
      selection = f.state.selection,
      history = undoDepth(f.state);
    return {
      f,
      base,
      fonts,
      root,
      input,
      context,
      media,
      paints,
      /** Observe all canonical callback roles without accessing private production fields. */
      observe(callback: typeof observe): void {
        observe = callback;
      },
      /** Public admission witnesses retirement before arbitrary external cleanup can reenter. */
      retired(): void {
        assert.equal(base.measure(input, context).kind, 'unavailable');
        assert.equal(base.current(input, context) === null, true);
      },
      /** Native font subscriptions retire exactly once even when later cleanup fails. */
      get fontRemoves() {
        return fontRemoves;
      },
      /** Resource retirement never changes CM-owned document, selection or history. */
      unchanged(): void {
        assert.equal(f.state.doc === source, true);
        assert.equal(f.state.selection === selection, true);
        assert.equal(undoDepth(f.state), history);
      },
      /** Independent finite owners settle even if another teardown throws. */
      destroy(): void {
        const failures: unknown[] = [];
        for (const release of [() => base.destroy(), () => f.destroy()]) {
          try {
            release();
          } catch (cause) {
            failures.push(cause);
          }
        }
        if (failures.length === 1) throw failures[0];
        if (failures.length) throw new AggregateError(failures, 'collision-fixture-cleanup-failed');
      },
    };
  } catch (cause) {
    const failures: unknown[] = [cause];
    for (const release of [() => base.destroy(), () => f.destroy()]) {
      try {
        release();
      } catch (cleanup) {
        failures.push(cleanup);
      }
    }
    if (failures.length === 1) throw cause;
    throw new AggregateError(failures, 'collision-fixture-acquisition-failed');
  }
}

test('terminal collision detaches before real media and fragment retirement; reentrant teardown is inert', () => {
  const x = retirementFixture(),
    operations: number[] = [],
    originalMedia = x.media.destroy,
    remove = x.root.remove;
  try {
    x.root.remove = () => {
      operations.push(0);
      x.retired();
      assert.equal(x.fontRemoves, 2);
      x.base.destroy();
      remove.call(x.root);
    };
    x.media.destroy = () => {
      operations.push(1);
      const connected = x.root.isConnected;
      originalMedia.call(x.media);
      assert.equal(connected, false);
      x.retired();
      x.base.destroy();
    };
    x.observe((role, node) => {
      operations.push(role + 2);
      x.retired();
      assert.equal(node.isConnected, false);
      x.base.destroy();
    });
    x.base.destroy();
    x.base.destroy();
    assert.deepEqual(operations, [0, 1, 2, 3, 4]);
    assert.equal(
      x.paints.every((paint) => paint.released === 1),
      true,
    );
    assert.equal(x.media.preflightAll().kind, 'retired');
    assert.equal(x.fontRemoves, 2);
    x.fonts.dispatchEvent(new Event('loadingdone'));
    x.retired();
    x.unchanged();
  } finally {
    x.destroy();
  }
});

for (const detachFirst of [false, true])
  test(`collision detach failure preserves ordered raw causes and every actual release, detached=${detachFirst}`, () => {
    const x = retirementFixture(),
      operations: number[] = [],
      originalMedia = x.media.destroy,
      remove = x.root.remove;
    const paintFailure = new Error('paint-retirement-control'),
      edgeFailure = new Error('edge-retirement-control');
    try {
      x.root.remove = () => {
        operations.push(0);
        x.retired();
        if (detachFirst) remove.call(x.root);
        // eslint-disable-next-line @typescript-eslint/only-throw-error -- Inject raw host failures to prove original cause retention and cleanup.
        throw null;
      };
      x.media.destroy = () => {
        operations.push(1);
        const connected = x.root.isConnected;
        originalMedia.call(x.media);
        assert.equal(connected, !detachFirst);
        x.retired();
        // eslint-disable-next-line @typescript-eslint/only-throw-error -- Inject raw host failures to prove original cause retention and cleanup.
        throw undefined;
      };
      x.observe((role) => {
        operations.push(role + 2);
        x.retired();
        if (role === 0) throw paintFailure;
        if (role === 1) throw edgeFailure;
      });
      let failure: unknown;
      try {
        x.base.destroy();
      } catch (cause) {
        failure = cause;
      }
      assert.equal(failure instanceof AggregateError, true);
      if (!(failure instanceof AggregateError)) throw new Error('collision-causes-missing');
      assert.equal(failure.errors.length, 4);
      assert.equal(failure.errors[0] === null, true);
      assert.equal(failure.errors[1] === undefined, true);
      assert.equal(failure.errors[2] === paintFailure, true);
      assert.equal(failure.errors[3] === edgeFailure, true);
      assert.deepEqual(operations, [0, 1, 2, 3, 4]);
      assert.equal(
        x.paints.every((paint) => paint.released === 1),
        true,
      );
      assert.equal(x.media.preflightAll().kind, 'retired');
      assert.equal(x.fontRemoves, 2);
      x.base.destroy();
      x.unchanged();
    } finally {
      remove.call(x.root);
      x.destroy();
    }
  });

test('a new collision mounted from old root removal survives remaining old-resource cleanup', () => {
  const x = retirementFixture(),
    remove = x.root.remove;
  try {
    x.root.remove = () => {
      x.retired();
      remove.call(x.root);
      assert.equal(x.base.prepare(x.input), true);
      x.base.mount(x.input, x.context);
    };
    x.base.destroy();
    const next = x.base.measure(x.input, x.context);
    assert.equal(next.kind, 'calibration');
    if (next.kind !== 'calibration') throw new Error('collision-renewal-missing');
    assert.equal(next.root !== x.root && next.root.isConnected, true);
    assert.equal(x.root.isConnected, false);
    assert.equal(x.paints[0]!.mounted, 2);
    assert.equal(x.paints[0]!.released, 1);
    x.unchanged();
  } finally {
    x.destroy();
  }
});
