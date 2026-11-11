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
/** Actual LoRA/Slot/Pending/carrier owners share finite native rectangle/style/media transports for candidate READ proof. */
import assert from 'node:assert/strict';
import { ReorderMountedLayout } from '../../../../../../frontend/sugarprompt/editor/src/reorder-mounted-layout.ts';
import { ReorderPassiveStage } from '../../../../../../frontend/sugarprompt/editor/src/reorder-motion-stage.ts';
import { preparePassiveMotion } from '../../../../../../frontend/sugarprompt/editor/src/reorder-passive-context.ts';
import { reorderMotionTargets } from '../../../../../../frontend/sugarprompt/editor/src/reorder-motion-targets.ts';
import { preparedLoraState } from './reorder-passive-reuse-fixture.ts';
import { wholeFixture } from './reorder-pointer-stage-fixture.ts';
import type { ReorderMountedCoverage } from '../../../../../../frontend/sugarprompt/editor/src/reorder-mounted-target.ts';

/** Independent native rectangles retain partially clipped held/static groups and one fully contained moving LoRA. */
export function candidateBox(left: number, width = 20): DOMRect {
  return {
    x: left,
    y: 20,
    left,
    right: left + width,
    top: 20,
    bottom: 40,
    width,
    height: 20,
    toJSON: () => ({ left, width }),
  };
}
/** Native rectangle lists retain ordinary iterable/item semantics without reproducing production reduction. */
export function candidateRects(rects: DOMRect[]): DOMRectList {
  return Object.assign(rects, {
    item: (index: number) => rects[index] ?? null,
  });
}
/** An independently authored pickup makes only contained peer1 translate; clipped peer2 stays unanimated. */
export function selectCandidatePeer(geometry: Parameters<typeof reorderMotionTargets>[0]) {
  return reorderMotionTargets(
    [
      { kind: 'chip', index: 1, left: 60, right: 80, top: 20, bottom: 40 },
      { kind: 'chip', index: 2, left: 230, right: 250, top: 20, bottom: 40 },
    ],
    geometry,
    null,
  );
}
/** The callback borrows genuine production owners and literal native observation witnesses, never a copied policy implementation. */
export interface CandidateReadFixture {
  readonly f: ReturnType<typeof wholeFixture>;
  readonly stage: ReorderPassiveStage;
  readonly child: NonNullable<ReturnType<ReorderPassiveStage['mount']>>;
  readonly prepared: NonNullable<ReturnType<typeof preparePassiveMotion>>;
  readonly roots: ReadonlyMap<number, HTMLElement>;
  readonly rich: ReadonlyMap<number, HTMLElement>;
  readonly coverages: ReorderMountedCoverage['kind'][];
  readonly rootReads: Map<number, number>;
  readonly richReads: Map<number, number>;
  readonly phase: () => ReorderMountedCoverage['kind'] | null;
  /** Actual borrowed-raster and native duplicate calls remain finite external observations. */
  readonly mediaCalls: () => {
    readonly hydrations: number;
    readonly copies: number;
  };
  /** Every changed external binding is registered before mutation and restored independently after the primary body cause. */
  readonly restore: (release: () => void) => void;
}
/** Whole-fit media uses the actual raster preflight; twenty512px rasters exercise unchanged viewport-only media fallback. */
export function usingCandidateReadFixture(
  action: (fixture: CandidateReadFixture) => void,
  options: {
    readonly atomic?: boolean;
    readonly viewportMedia?: boolean;
    readonly count?: 3 | 20;
    /** Baseline explicitly requests the original complete native observation at every READ. */
    readonly complete?: boolean;
  } = {},
): void {
  const previous = Object.getOwnPropertyDescriptor(globalThis, 'document'),
    observe = ReorderMountedLayout.prototype.observe,
    causes: unknown[] = [],
    releases: (() => void)[] = [];
  let f: ReturnType<typeof wholeFixture> | null = null,
    stage: ReorderPassiveStage | null = null;
  let hydrations = 0,
    copies = 0;
  try {
    f = wholeFixture(
      (document) =>
        Object.defineProperty(globalThis, 'document', {
          configurable: true,
          value: document,
        }),
      preparedLoraState(
        () => {
          hydrations++;
        },
        options.viewportMedia ? 20 : (options.count ?? 3),
        options.viewportMedia ? 512 : 16,
        options.viewportMedia ? 512 : 8,
      ),
    );
    const owned = f,
      nativeCreate = f.paint.document.createElement.bind(f.paint.document);
    f.paint.document.createElement = (tag) => {
      const node = nativeCreate(tag);
      if (tag === 'canvas')
        Object.assign(node, {
          getContext: () => ({
            /** Record the native duplicate without acquiring or interpreting media. */
            drawImage(_source: unknown) {
              copies++;
            },
          }),
        });
      return node;
    };
    const window = f.view.dom.ownerDocument.defaultView!,
      style = window.getComputedStyle;
    releases.push(() => {
      window.getComputedStyle = style;
    });
    window.getComputedStyle = (node, pseudo) => {
      const original = style(node, pseudo),
        values: Record<string, string> = {
          display:
            options.atomic === false
              ? 'inline'
              : node.classList.contains('sp-lora-chip')
                ? 'inline-flex'
                : node.classList.contains('sp-lora-weight')
                  ? 'inline-grid'
                  : 'inline',
          position: node.classList.contains('sp-reorder-chip') ? 'relative' : 'static',
          inset: '0px',
          transform: 'none',
          content: pseudo ? 'none' : 'normal',
        };
      return {
        // eslint-disable-next-line @typescript-eslint/no-misused-spread -- Native boundary fixtures intentionally project own fields without a browser class prototype.
        ...original,
        ...values,
        getPropertyValue: (name: string) => values[name] ?? original.getPropertyValue(name),
      } as CSSStyleDeclaration;
    };
    const prepared = preparePassiveMotion(f.view, 0);
    assert.ok(prepared?.whole);
    stage = new ReorderPassiveStage(f.view, prepared);
    const child = stage.mount(prepared);
    assert.ok(child);
    f.paint.admitLayer(stage);
    const placement = child.read(selectCandidatePeer);
    assert.equal(placement.kind, 'placement');
    if (placement.kind !== 'placement') assert.fail('candidate-native-placement-required');
    child.place(placement);
    const roots = new Map<number, HTMLElement>(),
      rich = new Map<number, HTMLElement>(),
      rootReads = new Map<number, number>(),
      richReads = new Map<number, number>(),
      coverages: ReorderMountedCoverage['kind'][] = [];
    let active: ReorderMountedCoverage['kind'] | null = null;
    for (const root of child.node.querySelectorAll<HTMLElement>('[data-reorder-chip]')) {
      const id = Number(root.dataset.reorderChip),
        left = id === 0 ? -5 : id === 2 ? 230 : id === 1 ? 20 : 260 + id * 40;
      roots.set(id, root);
      for (const element of [root, ...root.querySelectorAll<HTMLElement>('*')])
        element.getBoundingClientRect = () => candidateBox(left);
      const rectangle = root.getClientRects;
      root.getClientRects = () => {
        if (active) rootReads.set(id, (rootReads.get(id) ?? 0) + 1);
        return rectangle.call(root);
      };
      const island = root.querySelector<HTMLElement>('.sp-lora-chip')!;
      assert.ok(island);
      rich.set(id, island);
      const physical = island.getClientRects;
      island.getClientRects = () => {
        if (active) richReads.set(id, (richReads.get(id) ?? 0) + 1);
        return physical.call(island);
      };
    }
    releases.push(() => {
      ReorderMountedLayout.prototype.observe = observe;
    });
    ReorderMountedLayout.prototype.observe = function (targets, authority, readTargets, coverage) {
      if (options.complete) coverage = { kind: 'complete' };
      coverages.push(coverage.kind);
      active = coverage.kind;
      try {
        const observed = observe.call(this, targets, authority, readTargets, coverage);
        if (observed?.coverage !== 'carrier-candidates') return observed;
        return {
          ...observed,
          complete: () => {
            coverages.push('complete');
            active = 'complete';
            try {
              return observed.complete();
            } finally {
              active = null;
            }
          },
        };
      } finally {
        active = null;
      }
    };
    action({
      f: owned,
      stage,
      child,
      prepared,
      roots,
      rich,
      coverages,
      rootReads,
      richReads,
      phase: () => active,
      mediaCalls: () => ({ hydrations, copies }),
      restore: (release) => releases.push(release),
    });
  } catch (cause) {
    causes.push(cause);
  }
  for (const release of [
    ...releases.reverse(),
    () => stage?.destroy(),
    () => f?.destroy(),
    () => {
      if (previous) Object.defineProperty(globalThis, 'document', previous);
      else Reflect.deleteProperty(globalThis, 'document');
    },
  ])
    try {
      release();
    } catch (cause) {
      causes.push(cause);
    }
  if (causes.length === 1) throw causes[0];
  if (causes.length) throw new AggregateError(causes, 'candidate-read-fixture-retirement');
}
