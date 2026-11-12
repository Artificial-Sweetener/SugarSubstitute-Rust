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
/** Native phase boundaries keep viewport observations out of canonical WRITE, coalescing and numeric animation work. */
import assert from 'node:assert/strict';
import test from 'node:test';
import { undoDepth, redoDepth } from '@codemirror/commands';
import {
  initializeNativeStructure,
  prepareNativeStructure,
} from '../../../../../../frontend/sugarprompt/editor/src/structure.ts';
import { projectionState } from '../transaction/projection-state.ts';
import {
  prepareLoraPresentation,
  installLoraPresentation,
} from '../../../../../../frontend/sugarprompt/editor/src/lora-presentation.ts';
import { readLoraResources } from '../../../../../../frontend/sugarprompt/services/src/lora/validation.ts';
import { installLoraMedia } from '../../../../../../frontend/sugarprompt/editor/src/lora-media-state.ts';
import { editorFreshness } from '../../../../../../frontend/sugarprompt/editor/src/freshness.ts';
import { prepareProjection } from '../../../../../../frontend/sugarprompt/editor/src/projection-preparation.ts';
import { installProjection } from '../../../../../../frontend/sugarprompt/editor/src/rich-projection.ts';
import { reorderPreview } from '../../../../../../frontend/sugarprompt/editor/src/reorder-preview.ts';
import { readExactSource } from '../../../../../../frontend/sugarprompt/editor/src/exact-source.ts';
import { motionPresentationFixture } from './reorder-motion-presentation-fixture.ts';

/** Literal native property values belong to this external boundary, independently of readonly production leases. */
function guardViewport(f: ReturnType<typeof motionPresentationFixture>) {
  let phase: 'outside' | 'read' | 'write' | 'raf' = 'outside',
    reads = 0;
  const values = new Map<string, number>();
  for (const [target, properties] of [
    [f.view.scrollDOM, ['scrollLeft', 'scrollTop']],
    [
      f.view.dom.ownerDocument.defaultView!,
      ['scrollX', 'scrollY', 'innerWidth', 'innerHeight', 'devicePixelRatio'],
    ],
  ] as const) {
    for (const property of properties) {
      values.set(property, property === 'devicePixelRatio' ? 1 : 0);
      Object.defineProperty(target, property, {
        configurable: true,
        /** Geometry getters can force synchronous native layout and therefore reject every non-READ caller. */
        get() {
          assert.equal(phase, 'read', `${property}-outside-native-read`);
          reads++;
          return values.get(property);
        },
        /** Native values may change before an event is delivered; the following READ still must recertify them. */
        set(value: number) {
          values.set(property, value);
        },
      });
    }
  }
  return {
    /** One supported native request executes READ before its independently queued WRITE callback. */
    read() {
      assert.equal(phase, 'outside');
      phase = 'read';
      try {
        const job = f.jobs.shift();
        assert.ok(job);
        job();
      } finally {
        phase = 'outside';
      }
    },
    /** Hydration, supported publication and cleanup run without scalar geometry reads. */
    write() {
      assert.equal(phase, 'outside');
      phase = 'write';
      try {
        const task = f.tasks.shift();
        assert.ok(task);
        task();
      } finally {
        phase = 'outside';
      }
    },
    /** Numeric animation may consult source/resource leases but never native viewport geometry. */
    frame(now: number) {
      assert.equal(phase, 'outside');
      phase = 'raf';
      try {
        f.paint.step(now);
      } finally {
        phase = 'outside';
      }
    },
    /** This test intentionally models an unobserved native property change, rather than inventing a second invalidation owner. */
    change(property: string, value: number) {
      assert.ok(values.has(property));
      values.set(property, value);
    },
    /** Actual observed scroll/geometry refresh already cancels presentation through the native session measurement owner. */
    notify() {
      f.presentation.cancel();
    },
    /** Getter counts are diagnostic boundary evidence; actual mask/frame and hidden-child assertions own correctness. */
    count() {
      return reads;
    },
  };
}

/** Prepare a real canonical LoRA with finite injected pixels; normal and readonly widgets consume the same raster identity. */
function preparedLoraState(onPaint: () => void, count = 1, width = 16, height = 8) {
  let state = initializeNativeStructure(
    projectionState(
      count === 1
        ? 'a, <lora:fixture:0.70>, c'
        : Array.from({ length: count }, () => '<lora:fixture:0.70>').join(', '),
    ),
  );
  const resources = readLoraResources(
    [
      {
        name: 'fixture',
        resolution: 'found',
        model: {
          promptName: 'fixture',
          backendValue: 'fixture.safetensors',
          basename: 'fixture',
        },
        modelPageUrl: null,
        title: 'Fixture',
        subtitle: null,
        triggerWords: [],
        thumbnail: {
          status: 'ready',
          url: 'https://example.invalid/fixture.png',
          width,
          height,
        },
      },
    ],
    ['fixture'],
  );
  const resource = resources[0];
  assert.ok(resource?.resolution === 'found' && resource.thumbnail.status === 'ready');
  state = state.update({
    effects: installLoraPresentation.of(
      prepareLoraPresentation(state, { status: 'ready', resources }),
    ),
  }).state;
  state = state.update({
    effects: installLoraMedia.of({
      freshness: state.field(editorFreshness),
      media: new Map([
        [
          resource.thumbnail,
          {
            status: 'ready',
            raster: {
              width,
              height,
              /** Paint is an injected finite external callback; no acquisition or decode participates in the native phase proof. */
              paint: onPaint,
              /** The readonly painter never owns prepared raster disposal. */
              dispose() {
                assert.fail('borrowed-raster-disposal');
              },
            },
          },
        ],
      ]),
    }),
  }).state;
  return state.update({
    effects: installProjection.of(prepareProjection(state, prepareNativeStructure(state))),
  }).state;
}

for (const observed of ['scroll', 'resize', 'device-scale'] as const)
  test(`coalescing/WRITE/RAF avoid hostile viewport getters while observed ${observed} cancels real published mask`, () => {
    const f = motionPresentationFixture(),
      guard = guardViewport(f);
    try {
      f.prepare();
      f.prepare();
      guard.read();
      assert.equal(f.lastRead, 'capture');
      f.prepare();
      guard.write();
      for (const phase of ['layer', 'placement', 'ready'] as const) {
        f.prepare();
        guard.read();
        assert.equal(f.lastRead, phase);
        f.prepare();
        guard.write();
      }
      const child = f.view.dom.querySelector<HTMLElement>('.sp-reorder-motion-paint');
      assert.ok(child);
      assert.ok(f.state.field(reorderPreview).mask);
      f.prepare();
      guard.frame(90);
      assert.equal(child.dataset.reorderMotionProgress, '0.5');
      const reads = guard.count();
      guard.change(
        observed === 'scroll'
          ? 'scrollTop'
          : observed === 'resize'
            ? 'innerWidth'
            : 'devicePixelRatio',
        17,
      );
      guard.notify();
      // Resource retirement removes suppression immediately; its exact mask effect settles in the owned microtask.
      guard.write();
      assert.equal(guard.count(), reads);
      assert.equal(child.isConnected, false);
      assert.equal(f.state.field(reorderPreview).mask, null);
      assert.equal(f.frames.size, 0);
      assert.equal(readExactSource(f.state), 'a, b, c');
      assert.equal(undoDepth(f.state), 0);
      assert.equal(redoDepth(f.state), 0);
    } finally {
      f.destroy();
    }
  });

for (const change of [
  'stable-media',
  'silent-scroll',
  'setter-scroll',
  'observed-scroll',
  'raw-failure',
] as const)
  test(`actual prepared media WRITE ${change} cannot reveal or publish a stale canonical scene`, () => {
    const previous = Object.getOwnPropertyDescriptor(globalThis, 'document'),
      primary = new Error('controlled-native-raster-failure');
    let paintAction = () => {},
      paints = 0;
    const start = preparedLoraState(() => {
      paints++;
      paintAction();
    });
    let f: ReturnType<typeof motionPresentationFixture> | null = null;
    try {
      f = motionPresentationFixture(start, (document) =>
        Object.defineProperty(globalThis, 'document', {
          configurable: true,
          value: document,
        }),
      );
      const fixture = f,
        guard = guardViewport(fixture),
        selection = fixture.state.selection,
        initialPaints = paints;
      paintAction = () => {
        if (change === 'stable-media' || change === 'setter-scroll') return;
        if (change === 'raw-failure') throw primary;
        guard.change('scrollTop', 17);
        if (change === 'observed-scroll') guard.notify();
      };
      if (change === 'setter-scroll') {
        const create = fixture.paint.document.createElement.bind(fixture.paint.document);
        fixture.paint.document.createElement = (tag) => {
          const node = create(tag);
          if (tag === 'canvas') {
            let width = 0;
            Object.defineProperty(node, 'width', {
              configurable: true,
              /** Intrinsic output dimensions are independent of native viewport geometry. */
              get: () => width,
              /** A host-side native setter may change scroll before its actual event is delivered. */
              set(value: number) {
                width = value;
                if (value > 0) guard.change('scrollTop', 17);
              },
            });
          }
          return node;
        };
      }
      fixture.prepare();
      guard.read();
      if (change === 'raw-failure')
        assert.throws(
          () => guard.write(),
          (cause) => cause === primary,
        );
      else guard.write();
      assert.equal(paints, initialPaints + 1);
      assert.equal(fixture.state.field(reorderPreview).mask, null);
      const child = fixture.view.dom.querySelector<HTMLElement>('.sp-reorder-motion-paint');
      if (change === 'stable-media') {
        assert.ok(child);
        assert.equal(child.querySelectorAll('[data-reorder-motion-chip]').length, 0);
        // Initial WRITE hydrated real prepared pixels; the following native READ still owns calibration and viewport recertification.
        guard.read();
        assert.equal(fixture.lastRead, 'layer');
        guard.write();
        assert.equal(fixture.state.field(reorderPreview).mask, null);
        guard.read();
        assert.equal(fixture.lastRead, 'placement');
        guard.write();
        assert.equal(fixture.state.field(reorderPreview).mask, null);
        guard.read();
        assert.equal(fixture.lastRead, 'ready');
        guard.write();
        assert.ok(fixture.state.field(reorderPreview).mask);
        guard.frame(90);
        assert.equal(child.dataset.reorderMotionProgress, '0.5');
        guard.notify();
        guard.write();
        assert.equal(child.isConnected, false);
      } else if (change === 'silent-scroll' || change === 'setter-scroll') {
        assert.ok(child);
        assert.equal(fixture.jobs.length, 1);
        guard.read();
        assert.equal(fixture.lastRead, 'unavailable');
        guard.write();
        assert.equal(child.dataset.reorderMotionProgress, undefined);
        assert.equal(child.style.clip, 'rect(0px, 0px, 0px, 0px)');
        assert.equal(fixture.view.dom.querySelector('.sp-reorder-motion-stage'), null);
        assert.equal(fixture.view.dom.querySelectorAll('.sp-reorder-motion-stage').length, 0);
        assert.equal(child.isConnected, false);
        assert.equal(fixture.paint.layerMount.style.width, '0px');
        assert.equal(fixture.paint.layerMount.style.height, '0px');
        assert.equal(fixture.jobs.length + fixture.tasks.length, 0);
        assert.equal(child.querySelectorAll('[data-reorder-motion-chip]').length, 0);
      } else assert.equal(child, null);
      assert.equal(fixture.state.field(reorderPreview).mask, null);
      assert.equal(fixture.frames.size, 0);
      assert.equal(readExactSource(fixture.state), 'a, <lora:fixture:0.70>, c');
      assert.equal(fixture.state.selection, selection);
      assert.equal(undoDepth(fixture.state), 0);
      assert.equal(redoDepth(fixture.state), 0);
    } finally {
      try {
        f?.destroy();
      } finally {
        if (previous) Object.defineProperty(globalThis, 'document', previous);
        else Reflect.deleteProperty(globalThis, 'document');
      }
    }
  });

test('overbudget whole-mount media remains unplanned and uses the viewport WRITE followed by a real recertification READ', () => {
  const previous = Object.getOwnPropertyDescriptor(globalThis, 'document');
  let paints = 0;
  const start = preparedLoraState(
    () => {
      paints++;
    },
    20,
    512,
    512,
  );
  let f: ReturnType<typeof motionPresentationFixture> | null = null;
  try {
    f = motionPresentationFixture(start, (document) =>
      Object.defineProperty(globalThis, 'document', {
        configurable: true,
        value: document,
      }),
    );
    const fixture = f,
      guard = guardViewport(fixture),
      source = readExactSource(fixture.state),
      selection = fixture.state.selection,
      initial = paints;
    fixture.prepare();
    guard.read();
    guard.write();
    assert.equal(paints, initial);
    guard.read();
    assert.equal(fixture.lastRead, 'layer');
    guard.write();
    assert.equal(paints, initial);
    guard.read();
    assert.equal(fixture.lastRead, 'placement');
    guard.write();
    assert.equal(paints, initial);
    guard.read();
    assert.equal(fixture.lastRead, 'media');
    guard.write();
    assert.equal(paints, initial + 2);
    assert.equal(fixture.state.field(reorderPreview).mask, null);
    const child = fixture.view.dom.querySelector<HTMLElement>('.sp-reorder-motion-paint');
    assert.ok(child);
    assert.equal(child.querySelectorAll('[data-reorder-motion-chip]').length, 0);
    guard.read();
    guard.write();
    assert.ok(fixture.state.field(reorderPreview).mask);
    assert.equal(fixture.state.field(reorderPreview).mask!.ids.length, 2);
    guard.frame(90);
    assert.equal(child.dataset.reorderMotionProgress, '0.5');
    assert.equal(readExactSource(fixture.state), source);
    assert.equal(fixture.state.selection, selection);
    assert.equal(undoDepth(fixture.state), 0);
  } finally {
    try {
      f?.destroy();
    } finally {
      if (previous) Object.defineProperty(globalThis, 'document', previous);
      else Reflect.deleteProperty(globalThis, 'document');
    }
  }
});
