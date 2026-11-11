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
/** One canonical block reconciliation implementation retains exact resources and literal sibling order. */
import { requireValue } from '../../core/src/required.js';
import type { EditorView } from '@codemirror/view';
import type { ReorderPreview } from './reorder-preview-contracts.js';
import type { ReorderMotionMask } from './reorder-motion-mask.js';
import type { ReadonlyWidgetMedia } from './readonly-widget-media.js';
import type { ReorderLineLayout } from './reorder-line-layout.js';
import type { MountedReorderTarget } from './reorder-mounted-target.js';
import type { ReorderPaintAdmission } from './reorder-paint-admission.js';
import type { ReorderWidgetPlan } from './reorder-widget-plan.js';
import {
  ReorderMountedBlock,
  reorderMountedPlans,
  reorderMountedIdentity,
} from './reorder-widget-block.js';
import { createEmptyReorderTarget } from './reorder-native-targets.js';
import { presentReorderChip } from './reorder-chip-presentation.js';
/** These are the existing actual resources; Widget retains their registration, issue, completion and cleanup. */
interface MountedBlockResources {
  readonly blocks: Set<ReorderMountedBlock>;
  readonly targets: MountedReorderTarget[];
  readonly lines: ReorderLineLayout;
  readonly media: ReadonlyWidgetMedia | undefined;
}
/** Pure orchestration inputs delegate exact generation admission to the canonical mounted owner. */
interface ReconciliationPorts {
  readonly view: EditorView;
  readonly plan: ReorderWidgetPlan;
  readonly admission: ReorderPaintAdmission;
  current(): void;
}
/** Retain a largest already ordered subsequence of actual siblings; this request-local index owns no paint or plan authority. */
function retainedReorderSiblingIndices(
  root: HTMLElement,
  desired: readonly ReorderMountedBlock[],
): ReadonlySet<number> {
  const positions = new Map(Array.from(root.childNodes, (node, index) => [node, index] as const)),
    tails: number[] = [],
    tailIndices: number[] = [],
    predecessors = new Array<number>(desired.length).fill(-1);
  for (const [index, block] of desired.entries()) {
    const position = block.node ? positions.get(block.node) : undefined;
    if (position === undefined) continue;
    let lower = 0,
      upper = tails.length;
    while (lower < upper) {
      const middle = (lower + upper) >>> 1;
      if (requireValue(tails[middle], 'reorder-widget-blocks') < position) lower = middle + 1;
      else upper = middle;
    }
    if (lower > 0)
      predecessors[index] = requireValue(tailIndices[lower - 1], 'reorder-widget-blocks');
    tails[lower] = position;
    tailIndices[lower] = index;
  }
  const retained = new Set<number>();
  let index = tailIndices.at(-1) ?? -1;
  while (index >= 0) {
    retained.add(index);
    index = requireValue(predecessors[index], 'reorder-widget-blocks');
  }
  return retained;
}
/** Reconcile complete literal, chip, gap and marker output without retaining a second plan, roster or geometry cache. */
export function reconcileReorderBlocks(
  root: HTMLElement,
  mounted: MountedBlockResources,
  preview: ReorderPreview,
  mask: ReorderMotionMask | null,
  ports: ReconciliationPorts,
): void {
  const { view, plan, admission, current } = ports;
  current();
  const previous = new Map<string, ReorderMountedBlock[]>();
  for (const block of mounted.blocks) {
    const bucket = previous.get(block.identity) ?? [];
    bucket.push(block);
    previous.set(block.identity, bucket);
  }
  const selected = reorderMountedPlans(plan).map((block) => {
    const bucket = previous.get(reorderMountedIdentity(block));
    const retained = bucket?.find((candidate) => {
      const intact = candidate.intact();
      current();
      return intact;
    });
    if (retained)
      requireValue(bucket, 'reorder-widget-blocks').splice(
        requireValue(bucket, 'reorder-widget-blocks').indexOf(retained),
        1,
      );
    return { block, retained };
  });
  for (const bucket of previous.values())
    for (const block of bucket) {
      current();
      mounted.blocks.delete(block);
      block.destroy();
      current();
    }
  const desired: ReorderMountedBlock[] = [];
  let owner: ReorderMountedBlock;
  const paint = (target: HTMLElement, parts: Parameters<ReorderMountedBlock['paintParts']>[1]) =>
    owner.paintParts(target, parts, {
      view,
      plan,
      lines: mounted.lines,
      decorations: (part) => admission.decorations(part),
      media: mounted.media,
      current,
    });
  for (const { block, retained } of selected) {
    current();
    if (retained) {
      retained.metadata(block);
      if (block.kind === 'chip') {
        const node = retained.node as HTMLElement;
        presentReorderChip(node, block.id, preview, mask, current);
      }
      desired.push(retained);
      continue;
    }
    owner = new ReorderMountedBlock(block);
    mounted.blocks.add(owner);
    desired.push(owner);
    if (block.kind === 'literal') {
      owner.adopt(root.ownerDocument.createTextNode(block.value));
      current();
      owner.complete();
      continue;
    }
    if (block.kind === 'empty') {
      owner.adopt(createEmptyReorderTarget(root.ownerDocument).root);
      current();
      owner.complete();
      continue;
    }
    const wrapper = root.ownerDocument.createElement('span');
    owner.adopt(wrapper);
    current();
    if (block.kind === 'chip') {
      presentReorderChip(wrapper, block.id, preview, mask, current);
      current();
      wrapper.dataset.reorderChip = String(block.id);
      current();
      owner.targets.push({
        node: wrapper,
        kind: 'chip',
        index: block.id,
        ...paint(wrapper, block.parts),
      });
    } else {
      wrapper.className = 'sp-reorder-gap';
      current();
      for (const segment of block.segments) {
        const fragment = root.ownerDocument.createElement('span');
        wrapper.append(fragment);
        const painted = paint(fragment, segment.parts);
        if (segment.marker) {
          const marker = segment.marker,
            { root: anchor, target } = createEmptyReorderTarget(root.ownerDocument);
          if (marker.kind === 'partition') target.dataset.reorderPartition = String(marker.index);
          else {
            target.dataset.reorderGap = String(marker.index);
            target.dataset.reorderBlank = String(marker.blank);
          }
          wrapper.append(anchor);
          owner.targets.push({
            node: target,
            kind: marker.kind,
            index: marker.index,
            ...(marker.blank !== null ? { blank: marker.blank } : {}),
            ...painted,
          });
        }
      }
    }
    owner.complete();
  }
  current();
  const retained = retainedReorderSiblingIndices(root, desired);
  current();
  let anchor: ChildNode | null = null;
  for (let index = desired.length - 1; index >= 0; --index) {
    current();
    const node = requireValue(
      requireValue(desired[index], 'reorder-widget-blocks').node,
      'reorder-widget-blocks',
    );
    if (!retained.has(index)) root.insertBefore(node, anchor);
    current();
    anchor = node;
  }
  mounted.blocks.clear();
  for (const block of desired) mounted.blocks.add(block);
  mounted.targets.splice(0, mounted.targets.length, ...desired.flatMap((block) => block.targets));
  current();
}
