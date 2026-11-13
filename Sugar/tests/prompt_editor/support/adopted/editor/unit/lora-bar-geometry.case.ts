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
/** Actual CodeMirror LoRA styles and prepared widgets consume donor-backed geometry without another measuring owner. */
import assert from 'node:assert/strict';
import test, { mock } from 'node:test';
import { EditorState } from '@codemirror/state';
import { EditorView } from '@codemirror/view';
import { undoDepth } from '@codemirror/commands';
import { prepareWeightChange } from '../../../../../../frontend/sugarprompt/editor/src/weight-command.ts';
import { editorFreshness } from '../../../../../../frontend/sugarprompt/editor/src/freshness.ts';
import { readExactSource } from '../../../../../../frontend/sugarprompt/editor/src/exact-source.ts';
import { weightStateFor } from '../transaction/weight-command-state.ts';
import { loraTheme } from '../../../../../../frontend/sugarprompt/editor/src/lora-theme.ts';
import { LoraWidget } from '../../../../../../frontend/sugarprompt/editor/src/lora-widget.ts';
import { SourceToken } from '../../../../../../frontend/sugarprompt/editor/src/projection-widgets.ts';
import { LoraMediaPool } from '../../../../../../frontend/sugarprompt/editor/src/lora-media-pool.ts';
import type { LoraRaster } from '../../../../../../frontend/sugarprompt/editor/src/lora-media-contracts.ts';
import { readLoraResources } from '../../../../../../frontend/sugarprompt/services/src/lora/validation.ts';
import { prepareProjection } from '../../../../../../frontend/sugarprompt/editor/src/projection-preparation.ts';
import { prepareNativeStructure } from '../../../../../../frontend/sugarprompt/editor/src/structure.ts';
import { projectionState } from '../transaction/projection-state.ts';

/** Read a single publicly emitted theme rule without private CodeMirror extension fields. */
function rule(selector: string): string {
  const rules = EditorState.create({ extensions: [loraTheme] })
    .facet(EditorView.styleModule)
    .flatMap((module) => module.getRules().split('\n'));
  const matched = rules.filter(
    (value) => value.slice(0, value.indexOf(' {')).replace(/^\.\S+ /u, '') === selector,
  );
  assert.equal(matched.length, 1);
  return matched[0]!;
}

test('default LoRA bar reserves donor17px with one capped measured taper and seven-pixel interior', () => {
  const chip = rule('.sp-lora-chip');
  assert.match(chip, /--sp-lora-height: 17px;/u);
  assert.match(
    chip,
    /--sp-lora-taper: clamp\(4px, calc\(var\(--sp-lora-height\) \* .55\), 14px\);/u,
  );
  assert.match(chip, /height: var\(--sp-lora-height\);/u);
  assert.match(chip, /padding: 0 calc\(var\(--sp-lora-taper\) \+ 7px\);/u);
  assert.match(chip, /min-width: 120px; max-width: 540px;/u);
  for (const selector of ['.sp-lora-backing']) {
    assert.match(
      rule(selector),
      /polygon\(0 50%, var\(--sp-lora-taper\) 0, calc\(100% - var\(--sp-lora-taper\)\) 0, 100% 50%, calc\(100% - var\(--sp-lora-taper\)\) 100%, var\(--sp-lora-taper\) 100%\)/u,
    );
  }
});

test('title and numeric slot retain real native glyph sizing with donor gap and padding', () => {
  assert.match(rule('.sp-lora-title'), /flex: 1 1 auto; min-width: 0;/u);
  const weight = rule('.sp-lora-weight');
  assert.match(weight, /margin-left: 8px;/u);
  assert.match(weight, /--sp-weight-cell-padding: 4px;/u);
  assert.match(weight, /padding: 0 var\(--sp-weight-cell-padding\);/u);
  assert.doesNotMatch(weight, /min-width:/u);
  assert.match(rule('.sp-lora-weight .sp-weight-value'), /padding: 0;/u);
  const reservation = rule('.sp-lora-weight::before');
  assert.match(reservation, /content: '-8.88';/u);
  assert.match(reservation, /grid-area: 1 \/ 1;/u);
  assert.match(reservation, /visibility: hidden;/u);
  assert.match(reservation, /pointer-events: none;/u);
  assert.match(reservation, /user-select: none;/u);
});

/** Minimal literal external DOM transport; actual widget and weightValue construct every child. */
class ElementControl {
  width = 0;
  height = 0;
  className = '';
  textContent = '';
  title = '';
  readonly dataset: Record<string, string> = {};
  readonly attributes = new Map<string, string>();
  readonly children: ElementControl[] = [];
  /** Retain literal attributes from the production widget. */
  setAttribute(name: string, value: string): void {
    this.attributes.set(name, value);
  }
  /** Retain actual ordered production children without another rendering algorithm. */
  append(...children: ElementControl[]): void {
    this.children.push(...children);
  }
}

test('genuine prepared LoRA widget retains one literal numeric value without phantom source text', () => {
  const state = projectionState('<lora:missing:0.50>');
  const prepared = prepareProjection(state, prepareNativeStructure(state));
  const widgets: LoraWidget[] = [];
  prepared.all.between(0, state.doc.length, (_from, _to, decoration) => {
    if (decoration.spec.widget instanceof LoraWidget) widgets.push(decoration.spec.widget);
  });
  assert.equal(widgets.length, 1);
  const previous = Object.getOwnPropertyDescriptor(globalThis, 'document');
  Object.defineProperty(globalThis, 'document', {
    configurable: true,
    value: { createElement: () => new ElementControl() },
  });
  try {
    const chip = widgets[0]!.toDOM({} as EditorView) as unknown as ElementControl;
    const cell = chip.children.find((child) => child.className === 'sp-lora-weight');
    assert.ok(cell);
    assert.equal(cell.children.length, 1);
    const [value] = cell.children;
    assert.equal(value?.className, 'sp-weight-value');
    assert.equal(value?.textContent, '0.50');
    assert.equal(widgets[0]!.weight, '0.50');
    assert.equal(state.doc.toString(), '<lora:missing:0.50>');
  } finally {
    if (previous) Object.defineProperty(globalThis, 'document', previous);
    else Reflect.deleteProperty(globalThis, 'document');
  }
});

test('actual canonical exact commands independently map both wider token-end carets and one history event', () => {
  const source = '<lora:real:0.70>, <lora:missing:0.50>, following\nsecond';
  for (const example of [
    {
      range: { from: 0, to: 16 },
      end: 23,
      expected: '<lora:real:12345678.25>, <lora:missing:0.50>, following\nsecond',
    },
    {
      range: { from: 18, to: 37 },
      end: 44,
      expected: '<lora:real:0.70>, <lora:missing:12345678.25>, following\nsecond',
    },
  ]) {
    const state = weightStateFor(source, source.length);
    const result = prepareWeightChange(
      state,
      { kind: 'set', weight: '12345678.25' },
      {
        owner: 'exact',
        completion: 'keyboard',
        neutral: false,
        freshness: state.field(editorFreshness),
        kind: 'lora',
        range: example.range,
      },
    ).at(-1)!.state;
    assert.equal(readExactSource(result), example.expected);
    assert.deepEqual(
      [result.selection.main.anchor, result.selection.main.head],
      [example.end, example.end],
    );
    assert.equal(undoDepth(result), 1);
    assert.equal(readExactSource(state), source);
  }
});

test('compact numeric row stays intrinsic independently of the outer bar line-height', () => {
  assert.match(rule('.sp-lora-chip'), /line-height: var\(--sp-lora-height\);/u);
  assert.match(rule('.sp-lora-weight'), /line-height: normal;/u);
  assert.match(rule('.sp-lora-weight::before'), /grid-area: 1 \/ 1;/u);
  assert.match(rule('.sp-lora-weight .sp-weight-value'), /padding: 0;/u);
});

test('fresh LoRA canvases consume the same pool-prepared raster without decode, fetch or pixel retirement', async () => {
  const resources = readLoraResources(
    [
      {
        name: 'prepared',
        resolution: 'found',
        model: {
          promptName: 'prepared',
          backendValue: 'prepared.safetensors',
          basename: 'prepared',
        },
        modelPageUrl: null,
        title: 'Prepared model',
        subtitle: null,
        triggerWords: [],
        thumbnail: {
          status: 'ready',
          url: 'https://media.invalid/prepared',
          width: 16,
          height: 8,
        },
      },
    ],
    ['prepared'],
  );
  const resource = resources[0];
  assert.ok(resource?.resolution === 'found' && resource.thumbnail.status === 'ready');
  const reference = resource.thumbnail;
  const painted: HTMLCanvasElement[] = [];
  let decodes = 0,
    releases = 0;
  const raster: LoraRaster = {
    width: 16,
    height: 8,
    /** Retain the actual fresh canvas destinations for independent identity/size proof. */
    paint(canvas) {
      painted.push(canvas);
    },
    /** Only the real pool's retirement may release prepared pixels. */
    dispose() {
      releases++;
    },
  };
  let complete!: (value: LoraRaster) => void;
  const completion = new Promise<LoraRaster>((resolve) => {
    complete = resolve;
  });
  const pool = new LoraMediaPool(
    (input, signal) => {
      assert.equal(input, reference);
      assert.equal(signal.aborted, false);
      decodes++;
      return completion;
    },
    () => {},
    (cause) => {
      throw cause;
    },
  );
  const previous = Object.getOwnPropertyDescriptor(globalThis, 'document');
  const fetch = mock.method(globalThis, 'fetch', () => {
    throw new Error('paint-must-not-fetch');
  });
  let primary: { readonly cause: unknown } | null = null;
  try {
    pool.select([reference]);
    complete(raster);
    await completion;
    const prepared = pool.snapshot().get(reference);
    assert.ok(prepared?.status === 'ready');
    assert.equal(prepared.raster, raster);
    const widget = new LoraWidget(new SourceToken(), resource, '0.50', 'none', prepared);
    const equivalent = new LoraWidget(new SourceToken(), resource, '0.50', 'none', prepared);
    assert.equal(widget.eq(equivalent), true);
    Object.defineProperty(globalThis, 'document', {
      configurable: true,
      value: {
        /** Paint may allocate literal spans/canvases, never a network-bearing image transport. */
        createElement(name: string) {
          assert.ok(name === 'span' || name === 'canvas');
          return new ElementControl();
        },
      },
    });
    const first = widget.toDOM({} as EditorView) as unknown as ElementControl;
    const second = equivalent.toDOM({} as EditorView) as unknown as ElementControl;
    assert.notEqual(first, second);
    assert.equal(painted.length, 2);
    assert.notEqual(painted[0], painted[1]);
    for (const [index, chip] of [first, second].entries()) {
      const canvas = chip.children[0]?.children[0];
      assert.equal(Object.is(canvas, painted[index]), true);
      assert.deepEqual([canvas?.width, canvas?.height], [16, 8]);
      assert.equal(chip.dataset.media, 'ready');
      assert.equal(chip.dataset.resolution, 'found');
    }
    assert.equal(widget.resource, resource);
    assert.equal(widget.preparedMedia, prepared);
    assert.equal(equivalent.preparedMedia, prepared);
    assert.equal(pool.snapshot().get(reference), prepared);
    assert.equal(decodes, 1);
    assert.equal(fetch.mock.callCount(), 0);
    assert.equal(releases, 0);
  } catch (cause) {
    primary = { cause };
  }
  const causes: unknown[] = primary ? [primary.cause] : [];
  for (const cleanup of [
    () => {
      if (previous) Object.defineProperty(globalThis, 'document', previous);
      else Reflect.deleteProperty(globalThis, 'document');
    },
    () => fetch.mock.restore(),
    () => pool.dispose(),
  ]) {
    try {
      cleanup();
    } catch (cause) {
      causes.push(cause);
    }
  }
  if (causes.length === 1) throw causes[0];
  if (causes.length) throw new AggregateError(causes, 'prepared-widget-paint-cleanup-failed');
  assert.equal(releases, 1);
});
