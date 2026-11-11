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
import { drawNativeReorderMetadata } from './reorder-native-metadata-fixture.ts';
/** Actual rich projection/widget output characterizes whole-line admission without certifying native glyphs or input. */
import assert from 'node:assert/strict';
import { reorderMotionTargets } from '../../../../../../frontend/sugarprompt/editor/src/reorder-motion-targets.ts';
import type { ReorderMotionSelection } from '../../../../../../frontend/sugarprompt/editor/src/reorder-motion-scene.ts';
/** This low-level geometry proof deliberately selects every observed peer; Presentation separately proves changed-only policy. */
const selectAll: ReorderMotionSelection = (geometry) =>
  Array.from(
    new Map(
      geometry.map((rect) => [
        rect.index,
        {
          id: rect.index,
          kind: 'peer' as const,
          from: { left: rect.left, top: rect.top },
          to: { left: rect.left, top: rect.top },
        },
      ]),
    ).values(),
  );
import test from 'node:test';
import { StateEffect } from '@codemirror/state';
import { undoDepth, redoDepth } from '@codemirror/commands';
import { projectionState } from '../transaction/projection-state.ts';
import { captureReorder } from '../transaction/reorder-fixture.ts';
import {
  initializeNativeStructure,
  prepareNativeStructure,
} from '../../../../../../frontend/sugarprompt/editor/src/structure.ts';
import { installProjection } from '../../../../../../frontend/sugarprompt/editor/src/rich-projection.ts';
import { prepareProjection } from '../../../../../../frontend/sugarprompt/editor/src/projection-preparation.ts';
import {
  prepareLoraPresentation,
  installLoraPresentation,
} from '../../../../../../frontend/sugarprompt/editor/src/lora-presentation.ts';
import { readLoraResources } from '../../../../../../frontend/sugarprompt/services/src/lora/validation.ts';
import { prepareReorderPreview } from '../../../../../../frontend/sugarprompt/editor/src/reorder-preview-preparation.ts';
import {
  reorderPreviewExtension,
  installReorderPreview,
} from '../../../../../../frontend/sugarprompt/editor/src/reorder-preview.ts';
import { ReorderWidget } from '../../../../../../frontend/sugarprompt/editor/src/reorder-widget.ts';
import { ReorderPassiveStage } from '../../../../../../frontend/sugarprompt/editor/src/reorder-motion-stage.ts';
import { preparePassiveMotion } from '../../../../../../frontend/sugarprompt/editor/src/reorder-passive-context.ts';
import { admitReorderLineLayout } from '../../../../../../frontend/sugarprompt/editor/src/reorder-layout-admission.ts';
import { admitNativeReorderPaint } from '../../../../../../frontend/sugarprompt/editor/src/reorder-native-paint.ts';
import { captureReorderGrabPaint } from '../../../../../../frontend/sugarprompt/editor/src/reorder-grab-paint.ts';
import { readExactSource } from '../../../../../../frontend/sugarprompt/editor/src/exact-source.ts';
import { motionPaintFixture } from './reorder-motion-paint-fixture.ts';

for (const heldIndex of [0, 1, 2, 88, 89, 90])
  test(`45 repeated rich groups retain complete bounded held paint at index${heldIndex}`, () => {
    const source =
        '(soft light:2.00), <lora:fixture-found:0.70>, <lora:fixture-missing:0.50>, subject, '.repeat(
          45,
        ),
      paint = motionPaintFixture();
    let state = initializeNativeStructure(
      projectionState(source).update({
        effects: StateEffect.appendConfig.of(reorderPreviewExtension),
      }).state,
    );
    const resources = readLoraResources(
      [
        {
          name: 'fixture-found',
          resolution: 'found',
          model: {
            promptName: 'fixture-found',
            backendValue: 'fixture-found.safetensors',
            basename: 'fixture-found',
          },
          modelPageUrl: null,
          title: 'Fixture',
          subtitle: '',
          triggerWords: [],
          thumbnail: { status: 'unavailable' },
        },
        { name: 'fixture-missing', resolution: 'missing' },
      ],
      ['fixture-found', 'fixture-missing'],
    );
    state = state.update({
      effects: installLoraPresentation.of(
        prepareLoraPresentation(state, { status: 'ready', resources }),
      ),
    }).state;
    state = state.update({
      effects: installProjection.of(prepareProjection(state, prepareNativeStructure(state))),
    }).state;
    const capture = captureReorder(state),
      token = {},
      preview = prepareReorderPreview(
        {
          token,
          capture,
          activeChip: heldIndex,
          order: capture.prepared.initial,
        },
        'complete',
      );
    assert.ok(preview);
    state = state.update({ effects: installReorderPreview.of(preview) }).state;
    Object.defineProperty(paint.view, 'state', { get: () => state });
    let repeats = 1;
    let visibleCount = 32,
      partialId: number | null = null;
    const create = paint.document.createElement.bind(paint.document);
    paint.document.createElement = (tag) => {
      const node = create(tag),
        nativeRect = node.getBoundingClientRect.bind(node);
      /** Independent viewport rectangles expose complete visible groups while retaining the whole canonical source tree. */
      node.getBoundingClientRect = () => {
        let target: typeof node | null = node;
        while (target && target.dataset.reorderChip === undefined) target = target.parent;
        if (!target) return nativeRect();
        let scene: typeof node | null = target;
        while (scene && scene.className !== 'sp-reorder-motion-paint') scene = scene.parent;
        if (!scene) return nativeRect();
        const id = Number(target.dataset.reorderChip),
          top =
            id >= visibleCount || (id === partialId && node.className === 'sp-lora-title')
              ? 10000
              : 20;
        return {
          left: 20,
          right: 40,
          top,
          bottom: top + 20,
          x: 20,
          y: top,
          width: 20,
          height: 20,
        } as DOMRect;
      };
      Object.assign(node, {
        querySelector: (selector: string) =>
          node
            .querySelectorAll()
            .find((child) => child.className.split(/\s+/u).includes(selector.slice(1))) ?? null,
      });
      const nativeFragments = node.getClientRects.bind(node);
      /** A complete offscreen root already disqualifies this peer; its rich descendants must not incur geometry work. */
      node.getClientRects = () => {
        let target: typeof node | null = node;
        while (target && target.dataset.reorderChip === undefined) target = target.parent;
        let scene: typeof node | null = target;
        while (scene && scene.className !== 'sp-reorder-motion-paint') scene = scene.parent;
        if (
          scene &&
          target &&
          node !== target &&
          Number(target.dataset.reorderChip) >= visibleCount
        )
          throw new Error('offscreen-rich-descendant-geometry-read');
        const fragments = nativeFragments();
        return Array.from({ length: repeats }, () => fragments).flat();
      };
      return node;
    };
    const previous = Object.getOwnPropertyDescriptor(globalThis, 'document');
    Object.defineProperty(globalThis, 'document', {
      value: paint.document,
      configurable: true,
    });
    const widget = new ReorderWidget(preview, null);
    let root: HTMLElement | null = null;
    try {
      root = widget.toDOM(paint.view);
      paint.line.childNodes.splice(0);
      paint.line.append(root as unknown as typeof paint.line);
      const nativeRoot = root as unknown as typeof paint.line,
        targets = nativeRoot
          .querySelectorAll()
          .filter((node) => node.dataset.reorderChip !== undefined),
        held = targets[heldIndex]!;
      drawNativeReorderMetadata(paint, state, targets);
      assert.equal(targets.length, 180);
      assert.ok(held);
      const selected = admitNativeReorderPaint([held as unknown as HTMLElement]);
      assert.ok(selected);
      assert.ok(selected.length < 64);
      const nodes = [paint.line, ...paint.line.querySelectorAll()];
      assert.ok(nodes.length > 1024);
      assert.ok(admitReorderLineLayout([paint.line as unknown as HTMLElement]));
      const prepared = preparePassiveMotion(paint.view, heldIndex);
      assert.ok(prepared);
      if (heldIndex === 0) {
        const stage = new ReorderPassiveStage(paint.view, prepared);
        try {
          const child = stage.mount(prepared);
          assert.ok(child);
          paint.admitLayer(stage);
          const placement = child.read(selectAll);
          assert.equal(placement.kind, 'placement');
          if (placement.kind !== 'placement') assert.fail('native-calibration-required');
          child.place(placement);
          const ready = child.read(selectAll);
          assert.equal(ready.kind, 'ready');
          if (ready.kind !== 'ready') assert.fail('visible-canonical-groups-required');
          assert.equal(ready.scene.chips.length, 31);
          assert.equal(ready.scene.node.querySelectorAll('[data-reorder-chip]').length, 180);
          child.reveal();
          ready.scene.paint({
            progress: 0.5,
            positions: [{ id: 1, left: 30, top: 50 }],
          });
          assert.equal(ready.scene.chips[0]!.roots[0]!.node.style.top, '30px');
          child.destroy();
          partialId = 1;
          const partial = stage.mount(prepared);
          assert.ok(partial);
          const partialRead = partial.read(selectAll);
          assert.equal(partialRead.kind, 'ready');
          if (partialRead.kind !== 'ready') assert.fail('remaining-complete-groups-required');
          assert.equal(
            partialRead.scene.chips.some((chip) => chip.id === 1),
            false,
          );
          assert.equal(partialRead.scene.chips.length, 30);
          partial.destroy();
          partialId = null;
          visibleCount = 65;
          const boundary = stage.mount(prepared);
          assert.ok(boundary);
          const boundaryRead = boundary.read(selectAll);
          assert.equal(boundaryRead.kind, 'ready');
          if (boundaryRead.kind !== 'ready') assert.fail('64-visible-groups-required');
          assert.equal(boundaryRead.scene.chips.length, 64);
          boundary.destroy();
          visibleCount = 66;
          const excessive = stage.mount(prepared);
          assert.ok(excessive);
          assert.equal(excessive.read(selectAll).kind, 'unavailable');
          excessive.destroy();
          visibleCount = 131;
          const selected = stage.mount(prepared);
          assert.ok(selected);
          const selectedRead = selected.read((geometry) => {
            assert.equal(new Set(geometry.map((rect) => rect.index)).size, 130);
            const prior = geometry.map((rect) => ({
              ...rect,
              left: rect.left + (rect.index <= 2 ? 10 : 0),
              right: rect.right + (rect.index <= 2 ? 10 : 0),
            }));
            return reorderMotionTargets(prior, geometry, null);
          });
          assert.equal(selectedRead.kind, 'ready');
          if (selectedRead.kind !== 'ready') assert.fail('few-changed-rich-peers-required');
          assert.equal(new Set(selectedRead.scene.geometry.map((rect) => rect.index)).size, 130);
          assert.deepEqual(
            selectedRead.scene.chips.map((chip) => chip.id),
            [1, 2],
          );
          assert.deepEqual(
            selectedRead.scene.targets.map((target) => target.id),
            [1, 2],
          );
          selected.reveal();
          assert.equal(
            selectedRead.scene.node.querySelectorAll('[data-reorder-motion-chip]').length,
            2,
          );
          selected.destroy();
          assert.ok(selectedRead.scene.geometry.length * 8 > 4096);
          const fragmentCounts = new Map<number, number>();
          for (const rect of selectedRead.scene.geometry)
            fragmentCounts.set(rect.index, (fragmentCounts.get(rect.index) ?? 0) + 1);
          assert.ok(Array.from(fragmentCounts.values()).every((count) => count * 8 <= 256));
          repeats = 8;
          const excessFragments = stage.mount(prepared);
          assert.ok(excessFragments);
          assert.equal(
            excessFragments.read(() => assert.fail('aggregate-fragment-refusal-before-selection'))
              .kind,
            'unavailable',
          );
          excessFragments.destroy();
          repeats = 1;
        } finally {
          stage.destroy();
          visibleCount = 32;
          partialId = null;
        }
      }
      assert.equal(admitNativeReorderPaint([paint.line as unknown as HTMLElement]), null);
      // This controlled geometry transport exposes only the native fields consumed by grab capture.
      const view = {
        // eslint-disable-next-line @typescript-eslint/no-misused-spread -- Native boundary fixtures intentionally project own fields without a browser class prototype.
        ...paint.view,
        contentDOM: {
          querySelectorAll: (selector: string) =>
            selector === '.cm-line'
              ? [paint.line]
              : selector.startsWith('[data-reorder-chip')
                ? [held]
                : [],
          contains: (node: unknown) => node === paint.line,
        },
      } as unknown as typeof paint.view;
      const paintOutput = captureReorderGrabPaint(view, heldIndex, {
        x: 20,
        y: 10,
      });
      assert.ok(paintOutput);
      assert.equal(paintOutput.node.childNodes.length, 1);
      assert.equal(paintOutput.geometry.grabX, 20);
      assert.equal(paintOutput.geometry.grabY, 10);
      paintOutput.node.remove();
      assert.equal(readExactSource(state) === source, true);
      assert.equal(undoDepth(state), 0);
      assert.equal(redoDepth(state), 0);
      console.log(
        JSON.stringify({
          code: 'rich45-held-fallback-admitted',
          groups: 45,
          chips: targets.length,
          elementNodes: nodes.length,
          heldNodes: selected.length,
        }),
      );
    } finally {
      try {
        if (root) widget.destroy(root);
      } finally {
        if (previous) Object.defineProperty(globalThis, 'document', previous);
        else Reflect.deleteProperty(globalThis, 'document');
      }
    }
  });
