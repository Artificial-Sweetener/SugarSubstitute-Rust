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
/** One actual source/projection/canonical baseline and finite external DOM boundary support geometry-only collision media proof. */
import assert from 'node:assert/strict';
import { StateEffect, Transaction } from '@codemirror/state';
import { hideReorderChip } from '../../../../../../frontend/sugarprompt/core/src/index.ts';
import { readLoraResources } from '../../../../../../frontend/sugarprompt/services/src/lora/validation.ts';
import {
  installSemanticStructure,
  prepareNativeStructure,
} from '../../../../../../frontend/sugarprompt/editor/src/structure.ts';
import {
  loraPresentation,
  installLoraPresentation,
  prepareLoraPresentation,
} from '../../../../../../frontend/sugarprompt/editor/src/lora-presentation.ts';
import {
  loraMedia,
  installLoraMedia,
} from '../../../../../../frontend/sugarprompt/editor/src/lora-media-state.ts';
import type { LoraRaster } from '../../../../../../frontend/sugarprompt/editor/src/lora-media-contracts.ts';
import { editorFreshness } from '../../../../../../frontend/sugarprompt/editor/src/freshness.ts';
import { richRendering } from '../../../../../../frontend/sugarprompt/editor/src/presentation.ts';
import { sceneErrors } from '../../../../../../frontend/sugarprompt/editor/src/scene-errors.ts';
import {
  richProjectionExtension,
  installProjection,
} from '../../../../../../frontend/sugarprompt/editor/src/rich-projection.ts';
import { prepareProjection } from '../../../../../../frontend/sugarprompt/editor/src/projection-preparation.ts';
import { ReorderCollisionBase } from '../../../../../../frontend/sugarprompt/editor/src/reorder-collision-base.ts';
import { prepareReorderPreview } from '../../../../../../frontend/sugarprompt/editor/src/reorder-preview-preparation.ts';
import { ReorderWidget } from '../../../../../../frontend/sugarprompt/editor/src/reorder-widget.ts';
import { documentKind } from '../../../../../../frontend/sugarprompt/editor/src/document-kind.ts';
import { captureReorder } from '../transaction/reorder-fixture.ts';
import { fixture } from './reorder-session-fixture.ts';

/** Literal selector transport walks actual constructed nodes; it does not synthesize chrome, layout or rendering outcomes. */
function selectNodes(root: HTMLElement, selector: string): HTMLElement[] {
  const all: HTMLElement[] = [];
  for (const child of Array.from(root.childNodes))
    if (child.nodeType === 1) {
      const element = child as HTMLElement;
      all.push(element, ...selectNodes(element, '*'));
    }
  if (selector === '*') return all;
  if (selector.startsWith('.'))
    return all.filter((node) => node.className.split(/\s+/u).includes(selector.slice(1)));
  const match = /^\[data-([a-z-]+)(?:="([^"]*)")?\]$/u.exec(selector);
  if (!match) throw new TypeError('controlled-collision-selector');
  const property = match[1]!.replace(/-([a-z])/gu, (_pair: string, letter: string) =>
    letter.toUpperCase(),
  );
  return all.filter((node) => {
    const value: unknown = Reflect.get(node.dataset, property);
    return match[2] === undefined ? value !== undefined : value === match[2];
  });
}

/** Prepare real source/current projection/raster state through supported native effects; only external DOM and ready pixels are controlled. */
export function collisionFixture(edges = false) {
  const previous = Object.getOwnPropertyDescriptor(globalThis, 'document'),
    canvases: HTMLElement[] = [];
  let dimensionReads = 0,
    disposals = 0,
    allowPaint = true;
  const borrowed: {
    readonly raster: LoraRaster;
    readonly canvas: HTMLCanvasElement;
  }[] = [];
  const releases: (() => void)[] = [
    () => {
      if (previous) Object.defineProperty(globalThis, 'document', previous);
      else Reflect.deleteProperty(globalThis, 'document');
    },
  ];
  /** Retire every acquired resource independently; clearing the queue first fences cleanup reentry. */
  const release = (): void => {
    const actions = releases.splice(0),
      failures: unknown[] = [];
    for (const action of actions)
      try {
        action();
      } catch (cause) {
        failures.push(cause);
      }
    if (failures.length === 1) throw failures[0];
    if (failures.length) throw new AggregateError(failures, 'collision-fixture-retirement');
  };
  try {
    const f = fixture(undefined, (document) => {
      Object.defineProperty(globalThis, 'document', {
        configurable: true,
        value: document,
      });
      const create = document.createElement.bind(document);
      document.createElement = ((name: string) => {
        const node = create(name);
        // The existing session transport omits selector APIs; this exact external boundary observes only the real mounted tree.
        Object.defineProperties(node, {
          querySelectorAll: {
            value: (selector: string) => selectNodes(node, selector),
          },
          querySelector: {
            value: (selector: string) => selectNodes(node, selector)[0] ?? null,
          },
        });
        if (name === 'canvas') canvases.push(node);
        return node;
      }) as typeof create;
    });
    releases.unshift(() => f.destroy());
    const source = edges
      ? 'left,prompt,right\n"<lora:fixture:0.70>","center, other","<lora:fixture:0.70>"'
      : '(soft light:2.00), <lora:fixture:0.70>, <lora:missing:0.50>, subject, '.repeat(45);
    f.view.dispatch({
      changes: { from: 0, to: f.state.doc.length, insert: source },
      effects: StateEffect.appendConfig.of([
        documentKind.of(edges ? 'wildcard-csv' : 'prompt'),
        loraPresentation,
        loraMedia,
        sceneErrors,
        richRendering.of(true),
        richProjectionExtension,
      ]),
      annotations: Transaction.addToHistory.of(false),
    });
    f.view.dispatch({
      effects: installSemanticStructure.of({
        purpose: 'native',
        prepared: prepareNativeStructure(f.state),
      }),
    });
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
            width: 256,
            height: 256,
          },
        },
        { name: 'missing', resolution: 'missing' },
      ],
      ['fixture', 'missing'],
    );
    const found = resources[0];
    assert.ok(found?.resolution === 'found' && found.thumbnail.status === 'ready');
    const raster: LoraRaster = {
      /** Geometry-only rendering must not inspect intrinsic raster dimensions. */
      get width() {
        dimensionReads++;
        assert.equal(allowPaint, true, 'collision-raster-width-read');
        return 256;
      },
      /** The same borrowed dimensions serve the genuine visible native baseline. */
      get height() {
        dimensionReads++;
        assert.equal(allowPaint, true, 'collision-raster-height-read');
        return 256;
      },
      /** Actual visible canonical constructors must retain this exact borrowed raster receiver and distinct output buffers. */
      paint(canvas) {
        assert.equal(allowPaint, true, 'collision-raster-paint');
        assert.equal(this, raster);
        borrowed.push({ raster: this, canvas });
      },
      /** Only the prepared media pool owns disposal, never a collision or canonical widget mount. */
      dispose() {
        disposals++;
        assert.fail('borrowed-raster-disposal');
      },
    };
    f.view.dispatch({
      effects: installLoraPresentation.of(
        prepareLoraPresentation(f.state, { status: 'ready', resources }),
      ),
    });
    f.view.dispatch({
      effects: installLoraMedia.of({
        freshness: f.state.field(editorFreshness),
        media: new Map([[found.thumbnail, { status: 'ready', raster }]]),
      }),
    });
    f.view.dispatch({
      effects: installProjection.of(prepareProjection(f.state, prepareNativeStructure(f.state))),
    });
    const capture = captureReorder(f.state, edges ? source.indexOf('center') : 0),
      order = capture.prepared.initial,
      held = 0,
      hidden = hideReorderChip(capture.prepared, order, held);
    assert.ok(hidden);
    assert.equal(capture.prepared.chips.length, edges ? 2 : 180);
    const token = {},
      input = {
        token,
        capture,
        order,
        hidden,
        held,
        pointer: 7,
        coordinates: { x: 25, y: 20 },
      },
      preview = prepareReorderPreview({ token, capture, order, activeChip: held }, 'complete');
    assert.ok(preview);
    const widget = new ReorderWidget(preview, null),
      live = widget.toDOM(f.view);
    releases.unshift(
      () => widget.destroy(live),
      () => live.remove(),
    );
    f.view.dom.ownerDocument.body.append(live);
    assert.equal(borrowed.length, edges ? 0 : 45);
    assert.equal(canvases.length, edges ? 0 : 45);
    allowPaint = false;
    const base = new ReorderCollisionBase(f.view);
    releases.unshift(() => base.destroy());
    return {
      f,
      base,
      input,
      raster,
      live,
      widget,
      canvases,
      borrowed,
      get reads() {
        return dimensionReads;
      },
      get disposals() {
        return disposals;
      },
      /** Both real canonical owners release independently; prepared borrowed pixels remain untouched. */
      destroy: release,
    };
  } catch (primary) {
    try {
      release();
    } catch (cleanup) {
      throw new AggregateError([primary, cleanup], 'collision-fixture-construction');
    }
    throw primary;
  }
}
