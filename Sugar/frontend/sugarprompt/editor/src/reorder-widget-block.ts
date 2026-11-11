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
/** Complete canonical block identity and independently retired DOM/line resources serve live and passive widget updates. */
import { requireValue } from '../../core/src/required.js';
import type {
  ReorderPaintBlock,
  ReorderPaintPart,
  ReorderPaintLine,
  ReorderWidgetPlan,
} from './reorder-widget-plan.js';
import type { DecorationSet, EditorView } from '@codemirror/view';
import type { ReadonlyWidgetMedia } from './readonly-widget-media.js';
import type { ReorderLineLayout } from './reorder-line-layout.js';
import { renderReorderFragment, releaseReorderFragment } from './reorder-projection.js';
import type { ReorderLineLease } from './reorder-line-layout.js';
import type { MountedReorderTarget } from './reorder-mounted-target.js';
/** The block acquires its actual canonical line resources; Widget retains map/issue/layout authority. */
export interface ReorderBlockPaint {
  readonly view: EditorView;
  readonly plan: ReorderWidgetPlan;
  readonly lines: ReorderLineLayout;
  /** Consume the exact artifact classified by the canonical projection admission owner. */
  decorations(part: ReorderPaintLine): DecorationSet;
  readonly media: ReadonlyWidgetMedia | undefined;
  /** Reject synchronous replacement before another resource or DOM write. */
  current(): void;
}
/** Only canonical mount retirement produces this expected synchronous cancellation. */
export class ReorderPaintRetired extends Error {
  /** Preserve an explicit owner-issued retirement type across canonical resource boundaries. */
  constructor() {
    super('reorder-paint-retired');
    this.name = 'ReorderPaintRetired';
  }
}
/** Literal storage edges and the empty witness are complete canonical sibling blocks too. */
export type ReorderMountedPlan =
  | ReorderPaintBlock
  | {
      readonly kind: 'literal';
      readonly edge: 'prefix' | 'suffix';
      readonly value: string;
    }
  | {
      readonly kind: 'empty';
    };
/** Adapt the existing authoritative plan without deriving another carrier or rendering policy. */
export function reorderMountedPlans(plan: ReorderWidgetPlan): readonly ReorderMountedPlan[] {
  return [
    ...(plan.storage.prefix
      ? [
          {
            kind: 'literal' as const,
            edge: 'prefix' as const,
            value: plan.storage.prefix,
          },
        ]
      : []),
    ...plan.blocks,
    ...(plan.storage.suffix
      ? [
          {
            kind: 'literal' as const,
            edge: 'suffix' as const,
            value: plan.storage.suffix,
          },
        ]
      : []),
    ...(plan.empty ? [{ kind: 'empty' as const }] : []),
  ];
}
/** Content equality excludes shifted output coordinates and hard-line metadata, retaining exact ordered canonical fragment keys. */
function partIdentity(parts: readonly ReorderPaintPart[]): unknown {
  return parts.map((part) => (part.kind === 'text' ? ['text', part.value] : ['line', part.key]));
}
/** Occurrences consume separate mounted records even when their complete content identities coincide. */
export function reorderMountedIdentity(plan: ReorderMountedPlan): string {
  return JSON.stringify(
    plan.kind === 'chip'
      ? ['chip', plan.id, partIdentity(plan.parts)]
      : plan.kind === 'gap'
        ? ['gap', plan.segments.map((segment) => [partIdentity(segment.parts), segment.marker])]
        : plan,
  );
}
/** A block retains borrowed canonical paint until its exact removal, including partial construction and reentrant retirement. */
export class ReorderMountedBlock {
  readonly identity: string;
  readonly leases: ReorderLineLease[] = [];
  readonly targets: MountedReorderTarget[] = [];
  node: HTMLElement | Text | null = null;
  private retired = false;
  private completed = false;
  readonly plan: ReorderMountedPlan;
  /** Register this record before any external native allocation or canonical resource callback. */
  constructor(plan: ReorderMountedPlan) {
    this.plan = plan;
    this.identity = reorderMountedIdentity(plan);
  }
  /** Transfer one native allocation, or retire its late result after synchronous owner cancellation. */
  adopt(node: HTMLElement | Text): void {
    if (this.retired) {
      node.remove();
      throw new ReorderPaintRetired();
    }
    this.node = node;
  }
  /** Materialize complete literal/canonical parts through the original renderer and register every line before native append. */
  paintParts(
    target: HTMLElement,
    parts: readonly ReorderPaintPart[],
    ports: ReorderBlockPaint,
  ): {
    leases: ReorderLineLease[];
    origins: number[];
  } {
    const { view, plan, lines, decorations: projected, media, current } = ports;
    current();
    target.dataset.reorderFragment = 'true';
    const leases: ReorderLineLease[] = [],
      origins: number[] = [];
    for (const part of parts) {
      if (part.kind === 'text') {
        const ending = target.ownerDocument.createElement('span');
        ending.textContent = part.value;
        target.append(ending);
        continue;
      }
      origins.push(part.identity);
      current();
      const lease = lines.borrow(part.key, () => {
        const node = target.ownerDocument.createElement('span'),
          decorations = projected(part);
        renderReorderFragment(
          view,
          node,
          (range) =>
            plan.storage.literal({
              from: part.from + range.from,
              to: part.from + range.to,
            }),
          { from: 0, to: part.to - part.from },
          decorations,
          part.rowPaint,
          media,
        );
        return { node, destroy: () => releaseReorderFragment(node) };
      });
      this.borrow(lease);
      leases.push(lease);
      target.append(lease.node);
      current();
    }
    return { leases, origins };
  }
  /** Transfer borrowed output before any append can reenter the mounted owner. */
  borrow(lease: ReorderLineLease): void {
    if (this.retired) {
      lease.release();
      throw new ReorderPaintRetired();
    }
    this.leases.push(lease);
  }
  /** Only completely painted targets may be retained by a newer synchronous update. */
  complete(): void {
    if (this.retired) throw new ReorderPaintRetired();
    this.completed = true;
  }
  /** Detached inner paint cannot masquerade as an intact retained subtree. */
  intact(): boolean {
    const node = this.node;
    if (this.retired || !this.completed || !node) return false;
    for (const child of [
      ...this.leases.map((lease) => lease.node),
      ...this.targets.map((target) => target.node),
    ]) {
      const contained = node.contains(child);
      if (this.retired || this.node !== node || !contained) return false;
    }
    return true;
  }
  /** Refresh only derived hard-line metadata; retained actual nodes and borrowed line identities remain unchanged. */
  metadata(plan: ReorderMountedPlan): void {
    const parts =
      plan.kind === 'chip'
        ? [plan.parts]
        : plan.kind === 'gap'
          ? plan.segments.filter((segment) => segment.marker).map((segment) => segment.parts)
          : [];
    this.targets.splice(
      0,
      this.targets.length,
      ...this.targets.map((target, at) => ({
        ...target,
        origins: requireValue(parts[at], 'reorder-widget-block').flatMap((part) =>
          part.kind === 'line' ? [part.identity] : [],
        ),
      })),
    );
  }
  /** Retire ownership before all independent releases, preserving raw single or ordered combined causes. */
  destroy(): void {
    if (this.retired) return;
    this.retired = true;
    const leases = this.leases.splice(0),
      node = this.node,
      causes: unknown[] = [];
    this.node = null;
    this.targets.splice(0);
    for (const release of [...leases.map((lease) => () => lease.release()), () => node?.remove()]) {
      try {
        release();
      } catch (cause) {
        causes.push(cause);
      }
    }
    if (causes.length === 1) throw causes[0];
    if (causes.length) throw new AggregateError(causes, 'Reorder block retirement failed');
  }
}
