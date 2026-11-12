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
import { visibilityObserverFixture } from './reorder-visibility-observer-fixture.ts';
import { motionComputedStyleFixture } from './reorder-computed-style-fixture.ts';
import { ReorderMotionAttachment } from '../../../../../../frontend/sugarprompt/editor/src/reorder-motion-layer.ts';
import { PaintNode, PaintText } from './reorder-paint-node-fixture.ts';
/** Complete bounded native paint transport controls DOM/style reads; browser proof owns actual glyph shaping and pixels. */
import type { EditorView } from '@codemirror/view';
import type { PassiveMotionStage } from '../../../../../../frontend/sugarprompt/editor/src/reorder-motion-contracts.ts';
/** Only document allocation, native style and viewport line admission are faked; the production snapshot remains actual. */
export function motionPaintFixture() {
  let allocations = 0,
    styleReads = 0;
  let time = 0,
    nextFrame = 0;
  const frames = new Map<number, () => void>();
  const jobs: (() => void)[] = [],
    measurements = new Map<unknown, () => () => void>();
  let measureQueued = false;
  let attachment: ReorderMotionAttachment | null = null;
  let attachmentBatch: (() => void) | null = null;
  let measureJob: (() => void) | null = null;
  let readObserver: ((value: unknown, owner: 'layer' | 'preparation' | 'external') => void) | null =
    null;
  const preparationRequests = new WeakSet<object>();
  /** One supported native frame reads the complete current keyed batch before any measurement writes. */
  function requestMeasure<T>(request: {
    read(view: EditorView): T;
    write?(measured: T, view: EditorView): void;
    key?: unknown;
  }): void {
    if (request.key === attachment)
      attachmentBatch = () => {
        const measured = request.read(view);
        request.write?.(measured, view);
      };
    measurements.set(request.key ?? request, () => {
      const measured = request.read(view);
      readObserver?.(
        measured,
        request.key === attachment
          ? 'layer'
          : preparationRequests.has(request)
            ? 'preparation'
            : 'external',
      );
      return () => request.write?.(measured, view);
    });
    if (measureQueued) return;
    measureQueued = true;
    measureJob = () => {
      const batch = [...measurements.values()];
      if (!batch.length) throw new Error('fixture-empty-measure-frame');
      measurements.clear();
      measureQueued = false;
      const failures: unknown[] = [];
      const writes = batch.map((read) => {
        try {
          return read();
        } catch (cause) {
          failures.push(cause);
          return null;
        }
      });
      for (const write of writes)
        try {
          write?.();
        } catch (cause) {
          failures.push(cause);
        }
      if (failures.length === 1) throw failures[0];
      if (failures.length)
        throw new AggregateError(failures, 'fixture-native-measure-frame-failed');
    };
    jobs.push(measureJob);
  }
  const created: PaintNode[] = [];
  const document = {
    createElement(name: string): PaintNode {
      ++allocations;
      const node = new PaintNode(name, document, {
        left: 0,
        top: 0,
        width: 200,
        height: 20,
      });
      created.push(node);
      return node;
    },
    createTextNode(text: string) {
      ++allocations;
      return new PaintText(text);
    },
    createElementNS(_namespace: string, name: string): PaintNode {
      const node = this.createElement(name);
      node.namespaceURI = _namespace;
      return node;
    },
    head: null as PaintNode | null,
    body: null as PaintNode | null,
    defaultView: {
      getComputedStyle(node: PaintNode, pseudo?: string) {
        ++styleReads;
        return motionComputedStyleFixture(node, pseudo);
      },
      performance: { now: () => time },
      requestAnimationFrame(callback: () => void) {
        frames.set(++nextFrame, callback);
        return nextFrame;
      },
      cancelAnimationFrame(handle: number) {
        frames.delete(handle);
      },
    },
  };
  const nativeMutations = visibilityObserverFixture(document as unknown as Document);
  document.head = new PaintNode('head', document, {
    left: 0,
    top: 0,
    width: 0,
    height: 0,
  });
  document.body = new PaintNode('body', document, {
    left: 0,
    top: 0,
    width: 0,
    height: 0,
  });
  const line = new PaintNode('div', document, {
    left: 10,
    top: 20,
    width: 200,
    height: 20,
  });
  line.className = 'cm-line';
  const editor = new PaintNode('div', document, {
      left: 0,
      top: 0,
      width: 240,
      height: 100,
    }),
    scroller = new PaintNode('div', document, {
      left: 0,
      top: 0,
      width: 240,
      height: 100,
    }),
    content = new PaintNode('div', document, {
      left: 10,
      top: 20,
      width: 200,
      height: 20,
    });
  editor.className = 'cm-editor';
  scroller.className = 'cm-scroller';
  scroller.style.overflow = 'auto';
  content.className = 'cm-content';
  document.body.append(editor);
  editor.append(scroller);
  scroller.append(content);
  content.append(line);
  const chips = [0, 1, 2].map((id) => {
    const node = new PaintNode('span', document, {
      left: 20 + id * 40,
      top: 20,
      width: 20,
      height: 20,
    });
    node.dataset.reorderChip = String(id);
    node.className = 'sp-reorder-chip';
    node.append(new PaintText(['a', 'b', 'c'][id]!));
    line.append(node);
    return node;
  });
  const view = {
    dom: editor,
    root: document,
    domAtPos: () => ({ node: line, offset: 0 }),
    themeClasses: 'cm-editor',
    contentHeight: 20,
    scaleY: 1,
    scaleX: 1,
    dispatch: () => {},
    requestMeasure,
    contentDOM: content,
    scrollDOM: scroller,
  } as unknown as EditorView;
  attachment = new ReorderMotionAttachment(view);
  const layerRoot = scroller.querySelector('.sp-reorder-motion-layer');
  if (!layerRoot) throw new Error('fixture-native-attachment-required');
  /** Direct owner controls deliver only the actual keyed attachment request through its supported READ/WRITE pair. */
  function drawLayer(): void {
    if (!attachmentBatch) throw new Error('fixture-attachment-request-required');
    measurements.delete(attachment);
    attachmentBatch();
    if (!measurements.size && measureQueued) {
      const at = measureJob ? jobs.indexOf(measureJob) : -1;
      if (at >= 0) jobs.splice(at, 1);
      measureQueued = false;
    }
  }
  /** Geometry observation delegates to the genuine ViewPlugin update callback. */
  function scheduleLayer(): void {
    attachment!.update({ geometryChanged: true, viewportChanged: false });
  }
  const infrastructureAllocations = allocations;
  return {
    jobs,
    nativeMutations,
    /** Direct Stage proofs must admit their initially waiting marker through exactly one scheduled native batch. */
    admitLayer(stage: Pick<PassiveMotionStage, 'readLayer'>): void {
      if (stage.readLayer() !== 'waiting') throw new Error('fixture-layer-wait-required');
      const batch = jobs.shift();
      if (!batch) throw new Error('fixture-layer-frame-required');
      batch();
      if (stage.readLayer() !== 'ready') throw new Error('fixture-layer-admission-required');
    },
    /** Observe actual native READ outputs without invoking owner freshness or geometry again. */
    observeMeasureReads(
      observer: ((value: unknown, owner: 'layer' | 'preparation' | 'external') => void) | null,
    ): void {
      readObserver = observer;
    },
    /** Register the exact preparation request before delegating to the same supported native queue. */
    requestPreparationMeasure<T>(request: {
      read(view: EditorView): T;
      write?(measured: T, view: EditorView): void;
      key?: unknown;
    }): void {
      preparationRequests.add(request);
      view.requestMeasure(request);
    },
    attachment,
    layerRoot,
    layerMount: layerRoot.childNodes[0] as unknown as HTMLElement,
    drawLayer,
    scheduleLayer,
    /** Explicit layer teardown mirrors native extension/view retirement. */
    destroyLayer() {
      try {
        attachment!.destroy();
      } finally {
        measurements.clear();
        jobs.length = 0;
        measureQueued = false;
        readObserver = null;
      }
    },
    view,
    editor,
    scroller,
    content,
    line,
    chips,
    created,
    document,
    frames,
    get allocations() {
      return allocations - infrastructureAllocations;
    },
    get styleReads() {
      return styleReads;
    },
    text(value: string) {
      return new PaintText(value);
    },
    step(now: number) {
      time = now;
      const callbacks = [...frames.values()];
      frames.clear();
      for (const callback of callbacks) callback();
    },
  };
}
