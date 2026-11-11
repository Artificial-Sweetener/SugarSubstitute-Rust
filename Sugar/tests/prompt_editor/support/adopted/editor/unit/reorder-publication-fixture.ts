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
import { nativeReorderRootMetadata } from './reorder-native-metadata-fixture.ts';
/** Literal native publication/measurement transport observes actual full and hidden Core outputs. */
import assert from 'node:assert/strict';
import { EditorSelection, Transaction } from '@codemirror/state';
import { fixture } from './reorder-session-fixture.ts';
import { geometryUpdate } from './reorder-marker-fixture.ts';
import { reorderPreview } from '../../../../../../frontend/sugarprompt/editor/src/reorder-preview.ts';
import type { ReorderPreview } from '../../../../../../frontend/sugarprompt/editor/src/reorder-preview-contracts.ts';
import { prepareReorderPreview } from '../../../../../../frontend/sugarprompt/editor/src/reorder-preview-preparation.ts';
import { passiveGeometry } from './reorder-passive-geometry-fixture.ts';
import {
  exactReplacement,
  readExactSource,
} from '../../../../../../frontend/sugarprompt/editor/src/exact-source.ts';
import {
  valueReorderLayout,
  emptyValueReorderPartitions,
  hideReorderChip,
} from '../../../../../../frontend/sugarprompt/core/src/index.ts';
import {
  prepareNativeStructure,
  installSemanticStructure,
} from '../../../../../../frontend/sugarprompt/editor/src/structure.ts';

/** Publish current genuine source/semantics before entry, with literal line geometry following the actual full or hidden output. */
export function publicationFixture(source: string, position: number) {
  const f = fixture();
  f.view.dispatch({
    ...exactReplacement(f.state, 0, readExactSource(f.state).length, source),
    selection: EditorSelection.cursor(position),
    annotations: Transaction.addToHistory.of(false),
  });
  f.view.dispatch({
    effects: installSemanticStructure.of({
      purpose: 'native',
      prepared: prepareNativeStructure(f.state),
    }),
    annotations: Transaction.addToHistory.of(false),
  });
  /** The same literal physical transport observes either the genuine complete or canonical external collision output. */
  const targets = (published: ReorderPreview) => {
    const layout = valueReorderLayout(published.order),
      nativeValue = nativeReorderRootMetadata(f.state, f.view).get('data-reorder-native-value');
    /** Fixture geometry follows actual owner output and canonical carrier metadata, including real blank/partition targets. */
    const node = (dataset: Record<string, string>, offset: number, width: number) => ({
      dataset: {
        ...dataset,
        ...(nativeValue === undefined ? {} : { reorderNativeValue: nativeValue }),
      } as Readonly<Record<string, string>>,
      /** Literal native data attributes expose the same actual root-issued scope and local target identity as this geometry node. */
      getAttribute(name: string): string | null {
        if (!name.startsWith('data-')) return null;
        const key = name
          .slice(5)
          .replace(/-([a-z])/gu, (_match: string, letter: string) => letter.toUpperCase());
        return this.dataset[key] ?? null;
      },
      getClientRects() {
        const lines = published.output.text.slice(0, offset).split('\n'),
          left = 10 + lines.at(-1)!.length * 10,
          top = 10 + (lines.length - 1) * 30;
        return [
          {
            left,
            right: left + width,
            top,
            bottom: top + 20,
            width,
            height: 20,
          },
        ];
      },
    });
    const carrierStart = (carrier: 'prefix' | 'slot' | 'suffix', slot: number): number =>
      carrier === 'prefix'
        ? 0
        : carrier === 'suffix'
          ? (published.output.renderedRanges[published.order.order.at(-1)!]?.to ?? 0)
          : (published.output.renderedRanges[published.order.order[slot]!]?.to ?? 0);
    return [
      ...published.order.order.map((id) => {
        const range = published.output.renderedRanges[id]!;
        return node({ reorderChip: String(id) }, range.from, (range.to - range.from) * 10);
      }),
      ...layout.gaps.flatMap((g) =>
        g.blankOffsets.map((offset, blank) =>
          node(
            { reorderGap: String(g.index), reorderBlank: String(blank) },
            carrierStart(g.carrier, g.slot) + offset,
            1,
          ),
        ),
      ),
      ...emptyValueReorderPartitions(published.capture.prepared, published.order).map((p) =>
        node(
          { reorderPartition: String(p.partition) },
          carrierStart(p.carrier, p.slot) + p.offset,
          1,
        ),
      ),
    ];
  };
  f.view.contentDOM.querySelectorAll = ((selector: string) =>
    selector === '.cm-line'
      ? []
      : targets(
          f.state.field(reorderPreview).value!,
        )) as unknown as typeof f.view.contentDOM.querySelectorAll;
  f.setPassiveGeometry((node) => {
    if (node.className?.includes('sp-reorder-base-measurement')) return passiveGeometry(node);
    let root = node.parentNode;
    while (root && !root.className?.includes('sp-reorder-base-measurement')) root = root.parentNode;
    if (!root) return null;
    const full = f.state.field(reorderPreview).value!;
    const hidden = hideReorderChip(full.capture.prepared, full.order, full.activeChip)!;
    const collision = prepareReorderPreview({ ...full, order: hidden }, 'collision')!;
    const target = targets(collision).find((target) =>
      ['reorderChip', 'reorderGap', 'reorderBlank', 'reorderPartition'].every(
        (key) => target.dataset[key] === node.dataset?.[key],
      ),
    );
    return target ? (target.getClientRects() as DOMRect[]) : null;
  });
  let painted: object | null = null,
    notifications = 0;
  return {
    f,
    /** Observe actual external canonical targets rather than borrowing the complete live layout. */
    collisionTargets(): HTMLElement[] {
      const roots = Array.from(f.view.dom.ownerDocument.body.childNodes).filter((node) =>
        (node as HTMLElement).className.includes('sp-reorder-base-measurement'),
      );
      const result: HTMLElement[] = [];
      const visit = (node: Node): void => {
        const element = node as HTMLElement;
        if (
          element.dataset &&
          ['reorderChip', 'reorderGap', 'reorderPartition'].some(
            (key) => element.dataset[key] !== undefined,
          )
        )
          result.push(element);
        for (const child of Array.from(node.childNodes)) visit(child);
      };
      roots.forEach(visit);
      return result;
    },
    /** CM's pinned scheduler reads current geometry, notifies plugins, then executes the already-read write. */
    measure() {
      const job = f.jobs.shift();
      assert.ok(job);
      const result = job.read(f.view),
        published = f.state.field(reorderPreview).value?.order ?? null;
      if (published !== painted) {
        painted = published;
        notifications++;
        f.owner.update(geometryUpdate(f, true, false));
      }
      job.write?.(result, f.view);
    },
    /** Drain only actual owned tasks with a finite diagnostic bound; no clock or retry is synthesized. */
    settle() {
      for (let turn = 0; turn < 32 && (f.jobs.length || f.tasks.length); turn++) {
        if (f.jobs.length) this.measure();
        if (f.tasks.length) f.task();
      }
      assert.equal(f.jobs.length + f.tasks.length, 0);
    },
    get notifications() {
      return notifications;
    },
  };
}

/** Pump owned callbacks to actual external collision mounting and retain complete live publication for pending observation. */
export function hideReorderPublication(
  transport: ReturnType<typeof publicationFixture>,
): ReorderPreview {
  const { f } = transport;
  for (
    let turn = 0;
    turn < 16 &&
    !Array.from(f.view.dom.ownerDocument.body.childNodes).some((node) =>
      (node as HTMLElement).className.includes('sp-reorder-base-measurement'),
    );
    turn++
  ) {
    if (f.jobs.length) transport.measure();
    if (f.tasks.length) f.task();
  }
  const full = f.state.field(reorderPreview).value!;
  const hidden = hideReorderChip(full.capture.prepared, full.order, full.activeChip)!;
  const collision = prepareReorderPreview({ ...full, order: hidden }, 'collision')!;
  assert.equal(full.order.order.includes(full.activeChip), true);
  return collision;
}
