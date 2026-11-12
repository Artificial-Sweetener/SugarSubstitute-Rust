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
/** Actual CM capture/publication/widget authority is composed with the bounded native snapshot transport. */
import assert from 'node:assert/strict';
import type { ViewUpdate } from '@codemirror/view';
import { StateEffect, type TransactionSpec, type EditorState } from '@codemirror/state';
import { moveReorderChip } from '../../../../../../frontend/sugarprompt/core/src/index.ts';
import { currentNativeReorder } from '../../../../../../frontend/sugarprompt/editor/src/reorder-command.ts';
import {
  reorderPreview,
  reorderPreviewExtension,
  installReorderPreview,
} from '../../../../../../frontend/sugarprompt/editor/src/reorder-preview.ts';
import { prepareReorderPreview } from '../../../../../../frontend/sugarprompt/editor/src/reorder-preview-preparation.ts';
import { ReorderWidget } from '../../../../../../frontend/sugarprompt/editor/src/reorder-widget.ts';
import { ReorderMotionPresentation } from '../../../../../../frontend/sugarprompt/editor/src/reorder-motion-presentation.ts';
import type { PassiveMotionRead } from '../../../../../../frontend/sugarprompt/editor/src/reorder-motion-contracts.ts';
import { captureReorder, reorderState } from '../transaction/reorder-fixture.ts';
import { motionPaintFixture } from './reorder-motion-paint-fixture.ts';
/** The finite native transport distinguishes actual preparation outcomes from native layer and external checkpoints. */
type MotionReadObservation =
  | PassiveMotionRead['kind']
  | 'capture'
  | 'retired'
  | 'external'
  | 'native-layer';
/** Classify returned protocol shapes literally, without re-running resource admission or freshness predicates. */
function classifyMotionRead(
  value: unknown,
  owner: 'layer' | 'preparation' | 'external',
): MotionReadObservation {
  if (owner === 'layer') {
    assert.equal(Array.isArray(value), false, 'native-attachment-plan-required');
    assert.ok(value === null || (typeof value === 'object' && 'viewport' in value));
    return 'native-layer';
  }
  assert.equal(Array.isArray(value), false, 'foreign-native-read-array');
  if (owner === 'external') return 'external';
  if (value === null) return 'retired';
  assert.ok(value && typeof value === 'object');
  if ('prepared' in value) return value.prepared === null ? 'unavailable' : 'capture';
  assert.ok('kind' in value);
  const kind = value.kind;
  assert.ok(
    kind === 'layer' ||
      kind === 'placement' ||
      kind === 'media' ||
      kind === 'carriers' ||
      kind === 'ready' ||
      kind === 'unavailable',
  );
  return kind;
}
/** The native read/write and deferred queues remain explicit; no hidden editor or timing implementation is simulated. */
export function motionPresentationFixture(
  initialState: EditorState = reorderState('a, b, c').state,
  prepareDocument?: (document: Document) => void,
) {
  const paint = motionPaintFixture(),
    view = paint.view;
  prepareDocument?.(view.dom.ownerDocument);
  let state = initialState.update({
    effects: StateEffect.appendConfig.of(reorderPreviewExtension),
  }).state;
  const capture = captureReorder(state),
    token = {},
    order = moveReorderChip(capture.prepared, capture.prepared.initial, 0, {
      kind: 'row',
      row: 0,
      insertion: 1,
    });
  assert.ok(order);
  const preview = prepareReorderPreview({ token, capture, activeChip: 0, order }, 'complete');
  assert.ok(preview);
  state = state.update({ effects: installReorderPreview.of(preview) }).state;
  Object.defineProperty(view, 'state', { get: () => state });
  Object.assign(view, {
    dispatch(spec: TransactionSpec) {
      const previous = state.field(reorderPreview);
      const transaction = state.update(spec);
      state = transaction.state;
      const next = state.field(reorderPreview);
      const update: ViewUpdate = {
        view,
        state,
        startState: transaction.startState,
        transactions: [transaction],
        changes: transaction.changes,
        focusChanged: false,
        geometryChanged: transaction.docChanged,
        viewportChanged: false,
        viewportMoved: false,
        heightChanged: false,
        docChanged: transaction.docChanged,
        selectionSet: transaction.selection !== undefined,
      };
      paint.attachment.update(update);
      if (
        next.value === previous.value &&
        (next.mask !== previous.mask ||
          (previous.pointer?.mode === 'stage' && next.pointer?.mode === 'fallback')) &&
        next.decorations !== previous.decorations &&
        !next.native
      ) {
        next.decorations.between(0, state.doc.length, (_from, _to, decoration) => {
          const widget: unknown = decoration.spec.widget;
          if (widget instanceof ReorderWidget) assert.equal(widget.updateDOM(root, view), true);
        });
        // Pinned DocView.updateInner locks the content height only after supported Widget callbacks.
        view.contentDOM.style.height = `${view.contentHeight / view.scaleY}px`;
        // eslint-disable-next-line no-self-assign -- Exercise the native style setter even when its value is unchanged.
        view.contentDOM.style.flexBasis = view.contentDOM.style.flexBasis;
        view.contentDOM.style.height = '';
      }
    },
  });
  const widget = new ReorderWidget(preview, null),
    root = widget.toDOM(view);
  paint.line.childNodes.splice(0);
  paint.line.append(root as unknown as (typeof paint.chips)[number]);
  const targets = (root as unknown as (typeof paint.chips)[number])
    .querySelectorAll()
    .filter((node) => node.dataset.reorderChip !== undefined);
  assert.equal(targets.length, capture.prepared.chips.length);
  targets.forEach((node, index) =>
    Object.assign(node.box, {
      left: 20 + index * 40,
      top: 20,
      width: 20,
      height: 20,
    }),
  );
  // This transport supplies each mounted rich descendant's real admitted footprint, rather than the constructor's unlaid-out default box.
  for (const node of targets)
    for (const descendant of node.querySelectorAll()) Object.assign(descendant.box, node.box);
  const input = {
    token,
    capture,
    order,
    held: 0,
    hidden: null,
    pointer: null,
    coordinates: null,
  };
  // These independent external native rectangles match the already-mounted canonical order; actual browser proof owns layout.
  const nativeCreate = paint.document.createElement.bind(paint.document);
  paint.document.createElement = (name) => {
    const node = nativeCreate(name),
      nativeRect = node.getBoundingClientRect.bind(node);
    node.getBoundingClientRect = () => {
      let target: typeof node | null = node;
      while (target && target.dataset.reorderChip === undefined) target = target.parent;
      const id = target?.dataset.reorderChip;
      if (id !== undefined) {
        const left = id === '1' ? 20 : id === '0' ? 60 : 20 + Number(id) * 40;
        return {
          left,
          top: 20,
          right: left + 20,
          bottom: 40,
          x: left,
          y: 20,
          width: 20,
          height: 20,
        } as DOMRect;
      }
      return nativeRect();
    };
    return node;
  };
  let refreshes = 0,
    refresh: ((reason: 'geometry' | 'endpoint') => void) | null = null;
  const jobs = paint.jobs,
    tasks: (() => void)[] = [];
  let lastRead: MotionReadObservation | null = null,
    observeProgress: ((read: MotionReadObservation) => void) | null = null;
  let readSequence = 0;
  paint.observeMeasureReads((value, owner) => {
    ++readSequence;
    lastRead = classifyMotionRead(value, owner);
    observeProgress?.(lastRead);
  });
  const original = globalThis.queueMicrotask;
  globalThis.queueMicrotask = (task) => {
    tasks.push(task);
  };
  const presentation = new ReorderMotionPresentation(view, {
    current: (issued) => issued === input && currentNativeReorder(state, capture),
    rects: () =>
      targets.map((node) => ({
        kind: 'chip' as const,
        index: Number(node.dataset.reorderChip),
        left: node.box.left,
        right: node.box.left + node.box.width,
        top: node.box.top,
        bottom: node.box.top + node.box.height,
      })),
    refresh(_input, reason) {
      ++refreshes;
      refresh?.(reason);
    },
    observe: (action) => action(),
    schedule(request) {
      paint.requestPreparationMeasure(request);
    },
  });
  const origin = Object.freeze(
    capture.prepared.chips.map((_chip, id) =>
      Object.freeze({
        kind: 'chip' as const,
        index: id,
        left: 20 + id * 40,
        right: 40 + id * 40,
        top: 20,
        bottom: 40,
      }),
    ),
  );
  /** Complete the owned FIFO checkpoint before another native phase, failing rather than settling an unbounded chain. */
  function microtasks(): void {
    for (let delivered = 0; delivered < 128 && tasks.length; delivered++) {
      const task = tasks.shift();
      assert.ok(task);
      task();
    }
    assert.equal(tasks.length, 0, 'presentation-fixture-microtask-checkpoint-overflow');
  }
  return {
    ...paint,
    microtasks,
    paint,
    presentation,
    input,
    origin,
    view,
    root,
    widget,
    jobs,
    tasks,
    /** The exact last returned READ kind makes deliberately partial native frontiers inspectable. */
    get lastRead(): MotionReadObservation | null {
      return lastRead;
    },
    /** Observe the actual delegated invalidation owner without reproducing its policy. */
    get refreshes() {
      return refreshes;
    },
    /** An external native callback failure remains observable through the real presentation boundary. */
    onRefresh(callback: (reason: 'geometry' | 'endpoint') => void) {
      refresh = callback;
    },
    get state() {
      return state;
    },
    /** Publish a genuinely distinct canonical read-only issue under the same admitted source, rather than another observation of one issue. */
    publish() {
      const next = prepareReorderPreview(
        { token, capture, activeChip: input.held, order: input.order },
        'complete',
      );
      assert.ok(next);
      state = state.update({ effects: installReorderPreview.of(next) }).state;
    },
    prepare() {
      presentation.propose(input, origin);
    },
    /** Follow actual finite child progress; a terminal result leaves any genuinely new proposal for a separate explicit checkpoint. */
    deliver() {
      let captured = false,
        layers = 0,
        placed = false,
        hydrated = false,
        terminal = false,
        externalCheckpoint = false,
        nativeLayers = 0;
      assert.equal(observeProgress, null);
      observeProgress = (read) => {
        if (read === 'native-layer') {
          assert.equal(++nativeLayers, 1, 'duplicate-native-layer-issue');
          return;
        }
        if (read === 'external') {
          externalCheckpoint = true;
          return;
        }
        assert.equal(terminal, false, 'preparation-read-after-terminal');
        if (read === 'capture') {
          assert.equal(
            captured || layers > 0 || placed || hydrated,
            false,
            'new-capture-requires-explicit-checkpoint',
          );
          captured = true;
        } else if (read === 'layer') {
          assert.equal(placed || hydrated, false, 'layer-wait-after-placement');
          assert.ok(++layers <= 2, 'native-layer-wait-bound');
        } else if (read === 'placement') {
          assert.equal(placed || hydrated, false, 'duplicate-or-late-placement');
          placed = true;
        } else if (read === 'media') {
          assert.equal(hydrated, false, 'duplicate-media-hydration');
          hydrated = true;
        } else terminal = true;
      };
      try {
        while (jobs.length && !terminal && !externalCheckpoint) {
          const job = jobs.shift();
          assert.ok(job);
          const before = readSequence;
          job();
          assert.ok(readSequence > before, 'native-frame-without-read-progress');
          microtasks();
        }
      } finally {
        observeProgress = null;
      }
    },
    dispatch(spec: TransactionSpec) {
      view.dispatch(spec);
    },
    /** Attempt every owned native teardown and preserve raw ordered causes before restoring the global checkpoint. */
    destroy() {
      const failures: unknown[] = [];
      try {
        for (const release of [
          () => presentation.destroy(),
          () => widget.destroy(root),
          () => paint.destroyLayer(),
        ])
          try {
            release();
          } catch (cause) {
            failures.push(cause);
          }
      } finally {
        globalThis.queueMicrotask = original;
      }
      if (failures.length === 1) throw failures[0];
      if (failures.length)
        throw new AggregateError(failures, 'presentation-fixture-retirement-failed');
    },
  };
}
