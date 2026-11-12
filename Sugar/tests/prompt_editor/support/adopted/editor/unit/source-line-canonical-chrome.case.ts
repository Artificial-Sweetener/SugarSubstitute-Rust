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
/** SourceLines retains its actual containing native source-line chrome when a complete inline replacement removes all visible source ranges. */
import assert from 'node:assert/strict';
import test from 'node:test';
import { Compartment, EditorSelection, StateEffect } from '@codemirror/state';
import type { EditorView } from '@codemirror/view';
import { undoDepth, redoDepth } from '@codemirror/commands';
import { moveReorderChip } from '../../../../../../frontend/sugarprompt/core/src/index.ts';
import {
  SourceLines,
  sourceLineChrome,
} from '../../../../../../frontend/sugarprompt/editor/src/source-lines.ts';
import { readExactSource } from '../../../../../../frontend/sugarprompt/editor/src/exact-source.ts';
import {
  initializeNativeStructure,
  prepareNativeStructure,
} from '../../../../../../frontend/sugarprompt/editor/src/structure.ts';
import { installProjection } from '../../../../../../frontend/sugarprompt/editor/src/rich-projection.ts';
import { prepareProjection } from '../../../../../../frontend/sugarprompt/editor/src/projection-preparation.ts';
import { prepareReorderPreview } from '../../../../../../frontend/sugarprompt/editor/src/reorder-preview-preparation.ts';
import {
  reorderPreviewExtension,
  installReorderPreview,
  reorderPreview,
} from '../../../../../../frontend/sugarprompt/editor/src/reorder-preview.ts';
import { prepareReorderStoragePaint } from '../../../../../../frontend/sugarprompt/editor/src/reorder-storage-paint.ts';
import { ReorderWidget } from '../../../../../../frontend/sugarprompt/editor/src/reorder-widget.ts';
import { projectionState } from '../transaction/projection-state.ts';
import { captureReorder } from '../transaction/reorder-fixture.ts';

/** Each case fixes only prepared viewport/native measurement inputs; real native state, Core publication and source/history remain authoritative. */
interface ChromeAnchorCase {
  readonly csv: boolean;
  readonly enabled: boolean;
  readonly visible: boolean;
  readonly outside: boolean;
}
/** A finite owner consumes a genuine canonical publication and literal supported native metadata; layout access during decoration is forbidden. */
function proof(
  options: ChromeAnchorCase,
  body: (
    owner: SourceLines,
    measure: () => void,
    expectedLine: number,
    currentLine: number,
  ) => void,
): void {
  const source = options.csv
      ? 'prompt\n"shared\n[SEP|Face]\nfirst, second"\nelsewhere'
      : 'shared\n[SEP|Face]\nfirst, second',
    position = source.indexOf('first'),
    initial = initializeNativeStructure(
      projectionState(source, new Compartment(), options.csv ? 'wildcard-csv' : 'prompt').update({
        selection: EditorSelection.cursor(position),
      }).state,
    );
  let state = initial.update({
    effects: [
      installProjection.of(prepareProjection(initial, prepareNativeStructure(initial))),
      StateEffect.appendConfig.of([
        reorderPreviewExtension,
        sourceLineChrome.of({ enabled: options.enabled, contentLeftInset: 0 }),
      ]),
    ],
  }).state;
  const capture = captureReorder(state, position),
    order = moveReorderChip(capture.prepared, capture.prepared.initial, capture.active, {
      kind: 'row',
      row: 1,
      insertion: 1,
    });
  assert.ok(order);
  const preview = prepareReorderPreview(
    { capture, order, activeChip: capture.active, token: {} },
    'complete',
  );
  assert.ok(preview);
  state = state.update({ effects: installReorderPreview.of(preview) }).state;
  assert.equal(state.field(reorderPreview).native, false);
  const replacement = prepareReorderStoragePaint(preview).replacement,
    anchor = state.doc.lineAt(replacement.from),
    currentLine = state.doc.lineAt(position).number - 1;
  let widgets = 0;
  state.field(reorderPreview).decorations.between(0, state.doc.length, (from, to, value) => {
    if (value.spec.widget instanceof ReorderWidget) {
      widgets++;
      assert.deepEqual([from, to], [replacement.from, replacement.to]);
      assert.equal(value.spec.block === true, false);
      assert.equal(value.spec.widget.lineBreaks, 2);
    }
  });
  assert.equal(widgets, 1);
  const requests: Parameters<EditorView['requestMeasure']>[0][] = [];
  let reading = false;
  const view = {
    state,
    hasFocus: true,
    compositionStarted: false,
    scaleX: 1,
    scaleY: 1,
    documentTop: 100,
    visibleRanges: options.visible ? [{ from: anchor.from, to: anchor.from }] : [],
    /** Viewport is prepared numeric state; disabled chrome cannot discover a new native block or viewport. */
    get viewport() {
      if (!options.enabled) throw new Error('disabled-canonical-chrome-viewport-read');
      return options.outside
        ? { from: 0, to: source.indexOf('\n') }
        : { from: 0, to: state.doc.length };
    },
    /** Native line-block geometry belongs only to the existing scheduled READ. */
    get viewportLineBlocks() {
      if (!reading) throw new Error('canonical-chrome-layout-read-during-decoration');
      return [{ from: anchor.from, to: replacement.to, top: 20, height: 57 }];
    },
    scrollDOM: {
      clientWidth: 160,
      clientHeight: 120,
      getBoundingClientRect: () => ({ left: 20, top: 80 }),
    },
    requestMeasure(request: Parameters<EditorView['requestMeasure']>[0]) {
      requests.push(request);
    },
  } as unknown as EditorView;
  const owner = new SourceLines(view, () => 0, undefined),
    causes: unknown[] = [];
  /** The real queued read/write publishes the same containing native line index as its decoration. */
  const measure = () => {
    const request = requests.shift();
    assert.ok(request);
    reading = true;
    try {
      const value = request.read(view);
      request.write?.(value, view);
    } finally {
      reading = false;
    }
  };
  try {
    body(owner, measure, anchor.number - 1, currentLine);
    assert.equal(readExactSource(state), source);
    assert.deepEqual(
      [state.selection.main.anchor, state.selection.main.head, undoDepth(state), redoDepth(state)],
      [position, position, 0, 0],
    );
  } catch (cause) {
    causes.push(cause);
  }
  try {
    owner.destroy();
  } catch (cause) {
    causes.push(cause);
  }
  if (causes.length === 1) throw causes[0];
  if (causes.length) throw new AggregateError(causes, 'canonical-native-chrome-owner-retirement');
}

for (const csv of [false, true])
  for (const visible of [false, true])
    test(`canonical ${csv ? 'second-line CSV' : 'complete prompt'} containing native chrome survives ${visible ? 'deduplicated' : 'empty'} visible ranges`, () =>
      proof({ csv, enabled: true, visible, outside: false }, (owner, measure, line, current) => {
        const decorations: { from: number; className: unknown }[] = [];
        owner.decorations.between(0, 1000, (from, _to, value) => {
          decorations.push({ from, className: value.spec.class });
        });
        assert.deepEqual(decorations, [
          {
            from: csv ? 7 : 0,
            className: csv ? 'sp-source-line sp-source-alternate' : 'sp-source-line',
          },
        ]);
        assert.equal(line, csv ? 1 : 0);
        assert.equal(current, csv ? 3 : 2);
        assert.equal(owner.snapshot().currentLineIndex, current);
        measure();
        assert.equal(owner.snapshot().status, 'ready');
        assert.deepEqual(owner.snapshot().lines, [
          { lineIndex: line, top: 40, left: 0, width: 160, height: 57 },
        ]);
      }));

test('canonical native source-line chrome does not decorate an offviewport scope', () =>
  proof({ csv: true, enabled: true, visible: false, outside: true }, (owner) =>
    assert.equal(owner.decorations.size, 0),
  ));
test('disabled canonical native source-line chrome discovers no viewport or layout metadata', () =>
  proof({ csv: false, enabled: false, visible: false, outside: false }, (owner) =>
    assert.equal(owner.decorations.size, 0),
  ));
