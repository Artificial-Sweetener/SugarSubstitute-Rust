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
import { drain } from './reorder-marker-fixture.ts';
/** Controlled actual-CM session/measure lifetime proof; native DOM interaction remains browser-owned. */
import assert from 'node:assert/strict';
import test from 'node:test';

test('session disposal retires measurement policy listeners and captured resources after ordered native removal and release failures', () => {
  const listeners = new Set<string>(),
    cause = new Error('native-release-failed'),
    removal = new Error('native-paint-removal-failed');
  const f = fixture(undefined, (document) => {
    const media = {
      matches: false,
      addEventListener: () => listeners.add('media'),
      removeEventListener: () => listeners.delete('media'),
    } as unknown as MediaQueryList;
    document.defaultView!.matchMedia = () => media;
    Object.defineProperty(document, 'fonts', {
      value: {
        addEventListener: () => listeners.add('fonts'),
        removeEventListener: () => listeners.delete('fonts'),
      },
    });
  });
  try {
    assert.deepEqual([...listeners], ['media', 'fonts']);
    f.owner.pointerdown(pointer());
    f.measure();
    f.task();
    dragPointer(f.owner, pointer());
    drain(f);
    const surface = Array.from(f.view.dom.ownerDocument.body.childNodes).find((node) =>
      (node as HTMLElement).className.includes('sp-reorder-drag-surface'),
    ) as HTMLElement;
    assert.ok(surface);
    const remove = surface.remove;
    surface.remove = () => {
      Reflect.apply(remove, surface, []);
      throw removal;
    };
    const release = f.view.contentDOM.releasePointerCapture;
    f.view.contentDOM.releasePointerCapture = (id) => {
      Reflect.apply(release, f.view.contentDOM, [id]);
      throw cause;
    };
    f.owner.destroy();
    assert.equal(f.owner.blocksInput(), false);
    assert.throws(
      () => f.task(),
      (observed) =>
        observed instanceof AggregateError &&
        observed.errors[0] === removal &&
        observed.errors[1] === cause,
    );
    drain(f);
    assert.equal(f.state.field(reorderPreview).value, null);
    assert.equal(listeners.size, 0);
    assert.equal(f.frames.size, 0);
    assert.equal(f.captures.size, 0);
    assert.equal(f.owner.blocksInput(), false);
    assert.equal(
      Array.from(f.view.dom.ownerDocument.body.childNodes).filter((node) =>
        /sp-reorder-(?:motion-paint|landing|drag-surface|base-measurement|insertion-marker)/.test(
          (node as HTMLElement).className,
        ),
      ).length,
      0,
    );
  } finally {
    f.destroy();
  }
});
import { Transaction } from '@codemirror/state';
import type { EditorView } from '@codemirror/view';
import { undoDepth } from '@codemirror/commands';
import {
  reorderPreview,
  installReorderPreview,
} from '../../../../../../frontend/sugarprompt/editor/src/reorder-preview.ts';
import { readExactSource } from '../../../../../../frontend/sugarprompt/editor/src/exact-source.ts';
import {
  semanticStructures,
  prepareNativeStructure,
  installSemanticStructure,
} from '../../../../../../frontend/sugarprompt/editor/src/structure.ts';
import { dragPointer, fixture, key, pointer } from './reorder-session-fixture.ts';

test('actual preview is source neutral, Right follows Core adjacency and Alt release publishes one seeded genuine history edit', () => {
  const f = fixture();
  try {
    assert.equal(f.owner.keydown(key('Alt')), true);
    f.measure();
    assert.equal(f.state.field(reorderPreview).value, null);
    f.task();
    assert.equal(readExactSource(f.state) === 'a, b, c', true);
    assert.equal(undoDepth(f.state), 0);
    assert.equal(f.owner.keydown(key('ArrowRight')), true);
    drain(f);
    assert.equal(readExactSource(f.state) === 'a, b, c', true);
    f.owner.keyup(key('Alt'));
    assert.equal(readExactSource(f.state) === 'b, a, c', true);
    assert.equal(undoDepth(f.state), 1);
    assert.equal(f.state.field(reorderPreview).value, null);
    assert.equal(
      f.state.field(semanticStructures).native!.nativeValues.scopes.values.source ===
        readExactSource(f.state),
      true,
    );
    f.owner.keyup(key('Alt'));
    assert.equal(undoDepth(f.state), 1);
  } finally {
    f.destroy();
  }
});

test('Alt release during genuine arrow measurement waits for that output and Escape cancels without source history', () => {
  const f = fixture();
  try {
    f.owner.keydown(key('Alt'));
    f.measure();
    f.task();
    f.owner.keydown(key('ArrowRight'));
    f.owner.keyup(key('Alt'));
    drain(f);
    assert.equal(readExactSource(f.state) === 'b, a, c', true);
    assert.equal(undoDepth(f.state), 1);
    f.owner.keydown(key('Alt'));
    f.measure();
    f.task();
    f.owner.keydown(key('Escape'));
    assert.equal(f.state.field(reorderPreview).value, null);
    assert.equal(undoDepth(f.state), 1);
  } finally {
    f.destroy();
  }
});

test('source changes between read/write retire ownership without native update reentry and late callbacks cannot publish', () => {
  const f = fixture();
  try {
    f.owner.keydown(key('Alt'));
    f.measure();
    f.task();
    f.owner.keydown(key('ArrowRight'));
    const job = f.jobs.shift()!,
      read = job.read(f.view);
    f.view.dispatch({ changes: { from: 0, insert: 'x' } });
    job.write?.(read, f.view);
    while (f.tasks.length) f.task();
    assert.equal(readExactSource(f.state) === 'xa, b, c', true);
    assert.equal(f.state.field(reorderPreview).value, null);
    assert.equal(f.owner.blocksInput(), false);
  } finally {
    f.destroy();
  }
});

test('native capture/proxy and edge-scroll frame have finite cancellation and disposal lifetime', () => {
  const f = fixture();
  try {
    f.owner.pointerdown(pointer());
    drain(f);
    dragPointer(f.owner, pointer());
    // Deliver cold admission, then issue a genuine following move so proxy and edge-scroll each own a cancellable frame.
    f.measure();
    f.task();
    f.owner.pointermove(pointer(1, 20, 16));
    assert.equal(f.captures.has(1), true);
    assert.equal(f.frames.size, 2);
    f.owner.keydown(key('Escape'));
    assert.equal(f.captures.size, 0);
    assert.equal(f.frames.size, 0);
    assert.equal(f.removed, 1);
    const job = f.jobs.shift();
    if (job) {
      const value = job.read(f.view);
      job.write?.(value, f.view);
    }
    while (f.tasks.length) f.task();
    assert.equal(readExactSource(f.state) === 'a, b, c', true);
    assert.equal(undoDepth(f.state), 0);
  } finally {
    f.destroy();
  }
});

test('destroy during entry releases pending ownership and skips already-read deferred preparation', () => {
  const f = fixture();
  try {
    f.owner.keydown(key('Alt'));
    f.measure();
    f.owner.destroy();
    f.task();
    assert.equal(f.owner.blocksInput(), false);
    assert.equal(readExactSource(f.state) === 'a, b, c', true);
  } finally {
    f.destroy();
  }
});

test('matched pointerup implicit capture loss retains exactly the final measured drop and later release cannot duplicate it', () => {
  const f = fixture();
  try {
    f.owner.pointerdown(pointer());
    drain(f);
    dragPointer(f.owner, pointer());
    f.owner.pointerup(pointer(1, 20, 55));
    f.captures.delete(1);
    f.owner.pointerLost(pointer());
    drain(f);
    assert.equal(readExactSource(f.state) === 'b, c, a', true);
    assert.equal(undoDepth(f.state), 1);
    assert.equal(f.removed, 1);
    assert.equal(f.frames.size, 0);
    f.owner.keyup(key('Alt'));
    assert.equal(undoDepth(f.state), 1);
  } finally {
    f.destroy();
  }
});

test('measurement observation failure releases exact pending entry and preserves original cause', () => {
  const f = fixture(),
    cause = new Error('controlled-native-read-failure');
  try {
    f.view.posAtCoords = () => {
      throw cause;
    };
    f.owner.pointerdown(pointer());
    assert.throws(
      () => f.measure(),
      (error) => error === cause,
    );
    assert.equal(f.owner.blocksInput(), false);
    assert.equal(f.captures.size, 0);
    assert.equal(f.state.field(reorderPreview).value, null);
    f.view.posAtCoords = () => 0;
    drain(f);
    assert.equal(f.owner.keydown(key('Alt')), true);
    drain(f);
    assert.notEqual(f.state.field(reorderPreview).value, null);
  } finally {
    f.destroy();
  }
});

test('synchronous source observer reentry during preview publication retires remaining command and exact owned paint', () => {
  const f = fixture();
  try {
    f.observe((tx) => {
      if (tx.effects.some((effect) => effect.is(installReorderPreview))) {
        f.observe(null);
        f.view.dispatch({ changes: { from: 0, insert: 'x' } });
      }
    });
    f.owner.keydown(key('Alt'));
    f.measure();
    f.task();
    while (f.tasks.length) f.task();
    assert.equal(f.owner.blocksInput(), false);
    assert.equal(f.state.field(reorderPreview).value, null);
    f.owner.keyup(key('Alt'));
    assert.equal(readExactSource(f.state) === 'xa, b, c', true);
    assert.equal(undoDepth(f.state), 1);
  } finally {
    f.destroy();
  }
});

test('edge scroll writes one exact native step and leaving the edge retires its free-running frame', () => {
  const f = fixture();
  try {
    f.setChipGeometry(0, 15, 119);
    f.owner.pointerdown(pointer(1, 119));
    drain(f);
    dragPointer(f.owner, pointer(1, 119));
    drain(f);
    f.frame(30);
    f.measureFrame();
    assert.equal(f.view.scrollDOM.scrollTop, 24);
    assert.equal(f.frames.size, 1);
    f.owner.pointermove(pointer(1, 60));
    f.frame(60);
    f.measure();
    f.task();
    f.measure();
    assert.equal(f.frames.size, 0);
    f.owner.close();
    assert.equal(f.captures.size, 0);
    assert.equal(f.state.field(reorderPreview).value, null);
  } finally {
    f.destroy();
  }
});

test('native capture failure releases primed pointer capture without creating a proxy and preserves its exact cause before a genuine following entry', () => {
  const f = fixture(),
    cause = new Error('controlled-native-capture-failure'),
    capture = f.view.contentDOM.setPointerCapture;
  try {
    f.view.contentDOM.setPointerCapture = () => {
      throw cause;
    };
    f.owner.pointerdown(pointer());
    f.measure();
    assert.throws(
      () => f.task(),
      (error) => error === cause,
    );
    while (f.tasks.length) f.task();
    assert.equal(f.owner.blocksInput(), false);
    assert.equal(f.captures.size, 0);
    assert.equal(f.frames.size, 0);
    assert.equal(f.removed, 0);
    assert.equal(f.state.field(reorderPreview).value, null);
    f.view.contentDOM.setPointerCapture = capture;
    f.owner.keydown(key('Alt'));
    f.measure();
    f.task();
    assert.notEqual(f.state.field(reorderPreview).value, null);
  } finally {
    f.destroy();
  }
});

test('keyboard entry captures genuine settled native output while an old pointer hit refuses changed geometry', () => {
  const settle = (view: EditorView): void => {
    const changed = view.state.update({
      changes: { from: 0, to: 1, insert: '(a:1)' },
      selection: { anchor: 1 },
      annotations: Transaction.addToHistory.of(false),
    });
    view.dispatch(changed);
    view.dispatch({
      effects: installSemanticStructure.of({
        purpose: 'native',
        prepared: prepareNativeStructure(view.state),
      }),
      annotations: Transaction.addToHistory.of(false),
    });
  };
  const keyboard = fixture(settle);
  try {
    keyboard.owner.keydown(key('Alt'));
    keyboard.measure();
    keyboard.task();
    const preview = keyboard.state.field(reorderPreview).value!;
    assert.notEqual(preview, null);
    assert.equal(preview.capture.target.scope.document.sourceText === '(a:1), b, c', true);
    assert.equal(keyboard.state.selection.main.head, 1);
    assert.equal(undoDepth(keyboard.state), 0);
  } finally {
    keyboard.destroy();
  }
  const mouse = fixture(settle);
  try {
    mouse.owner.pointerdown(pointer());
    mouse.measure();
    mouse.task();
    dragPointer(mouse.owner, pointer());
    assert.equal(mouse.owner.blocksInput(), false);
    assert.equal(mouse.state.field(reorderPreview).value, null);
    assert.equal(mouse.captures.size, 0);
  } finally {
    mouse.destroy();
  }
});

for (const arrows of [
  ['ArrowRight', 'ArrowRight'],
  ['ArrowRight', 'ArrowLeft', 'ArrowRight'],
] as const)
  test(`Alt release drains all ${arrows.length} measured arrow intents in order`, () => {
    const f = fixture();
    try {
      f.owner.keydown(key('Alt'));
      f.measure();
      f.task();
      for (const arrow of arrows) f.owner.keydown(key(arrow));
      f.owner.keyup(key('Alt'));
      for (const [index] of arrows.entries()) {
        const previous = f.state.field(reorderPreview).value;
        for (let turn = 0; turn < 32 && f.state.field(reorderPreview).value === previous; turn++) {
          if (f.jobs.length) f.measure();
          if (f.tasks.length) f.task();
        }
        assert.notEqual(f.state.field(reorderPreview).value, previous);
        if (index < arrows.length - 1) {
          assert.equal(readExactSource(f.state), 'a, b, c');
          assert.equal(undoDepth(f.state), 0);
        }
      }
      drain(f);
      assert.equal(readExactSource(f.state), arrows.length === 2 ? 'b, c, a' : 'b, a, c');
      assert.equal(undoDepth(f.state), 1);
      assert.equal(f.owner.blocksInput(), false);
      assert.equal(f.state.field(reorderPreview).value, null);
    } finally {
      f.destroy();
    }
  });

test('the explicit pending-arrow bound consumes overflow and cancels the complete gesture without publishing source or selection', () => {
  const f = fixture(),
    original = f.state.selection;
  try {
    f.owner.keydown(key('Alt'));
    f.measure();
    f.task();
    assert.equal(f.owner.keydown(key('ArrowRight')), true);
    for (let index = 0; index < 256; index++)
      assert.equal(f.owner.keydown(key(index % 2 ? 'ArrowLeft' : 'ArrowRight')), true);
    assert.equal(f.owner.blocksInput(), true);
    assert.equal(f.owner.keydown(key('ArrowRight')), true);
    assert.equal(f.owner.blocksInput(), false);
    assert.equal(f.state.field(reorderPreview).value, null);
    assert.equal(readExactSource(f.state), 'a, b, c');
    assert.equal(f.state.selection.eq(original), true);
    assert.equal(undoDepth(f.state), 0);
    while (f.jobs.length || f.tasks.length) {
      if (f.jobs.length) f.measure();
      if (f.tasks.length) f.task();
    }
    f.owner.keyup(key('Alt'));
    assert.equal(readExactSource(f.state), 'a, b, c');
    assert.equal(undoDepth(f.state), 0);
    assert.equal(f.captures.size, 0);
    assert.equal(f.frames.size, 0);
  } finally {
    f.destroy();
  }
});
