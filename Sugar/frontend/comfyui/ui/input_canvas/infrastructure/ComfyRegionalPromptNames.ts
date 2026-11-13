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
/** Propagate separator-name edits between the live prompts related to one regional mask loader. */
import type { ComfyGraph, ComfyNode, ComfyWidget } from '../../types/graph.js';
import type { WorkflowRegionalPrompts } from '../application/WorkflowRegionalPrompts.js';
import { synchronizeRegionalPromptNames } from '../domain/RegionalPromptNames.js';
import { promptField } from './ComfyRegionalPrompts.js';

/**
 * Watch the text of every prompt related to an ordered mask loader and, when one changes, give
 * all prompts of that loader the same separator names.
 *
 * The first reading after a workflow loads only records the authored text, so opening a workflow
 * never modifies it. Every rewrite is committed as the user's own edit would be, inside one
 * native history step; a rewrite caused by a prompt edit joins that edit's step.
 */
export class ComfyRegionalPromptNames {
  readonly #stop: () => void;
  #texts = new Map<ComfyNode, string>();
  #writing = false;

  /** Observe regional relations and commit through the workflow's root graph history. */
  constructor(
    private readonly regions: Pick<WorkflowRegionalPrompts, 'masks' | 'subscribe'>,
    private readonly node: (path: string) => ComfyNode | null,
    private readonly root: () => ComfyGraph | null | undefined,
    private readonly report: (error: unknown) => void,
  ) {
    this.#stop = regions.subscribe(() => this.#reconcile());
  }

  /** Stop observing and forget recorded prompt text. */
  dispose(): void {
    this.#stop();
    this.#texts.clear();
  }

  /** Record current prompt text and synchronize every loader whose prompts were edited. */
  #reconcile(): void {
    if (this.#writing) return;
    try {
      const recorded = this.#texts,
        current = new Map<ComfyNode, string>();
      const groups = this.regions.masks().map((mask) =>
        mask.prompts.flatMap((path) => {
          const node = this.node(path),
            field = node ? promptField(node) : null;
          if (!node || !field) return [];
          current.set(node, field.value);
          return [{ node, widget: field.widget, text: field.value }];
        }),
      );
      this.#texts = current;
      const writes = new Map<ComfyNode, { widget: ComfyWidget; text: string }>();
      for (const group of groups) {
        if (group.length < 2) continue;
        const sources = group.map(({ node, text }) => {
          const previous = recorded.get(node);
          return { text, previous: previous !== undefined && previous !== text ? previous : null };
        });
        if (!sources.some((source) => source.previous !== null)) continue;
        for (const [index, text] of synchronizeRegionalPromptNames(sources).entries()) {
          const prompt = group[index];
          if (prompt && text !== null) writes.set(prompt.node, { widget: prompt.widget, text });
        }
      }
      if (writes.size) this.#commit(writes);
    } catch (error: unknown) {
      this.report(error);
    }
  }

  /** Write every rewritten prompt as one native history step without observing its own writes. */
  #commit(writes: ReadonlyMap<ComfyNode, { widget: ComfyWidget; text: string }>): void {
    for (const [node, { text }] of writes) this.#texts.set(node, text);
    const graph = this.root();
    this.#writing = true;
    graph?.beforeChange?.();
    try {
      for (const [node, { widget, text }] of writes) {
        const previous = widget.value;
        widget.value = text;
        widget.callback?.(text);
        if (typeof node.onWidgetChanged === 'function')
          node.onWidgetChanged.call(node, widget.name, text, previous, widget);
      }
      graph?.setDirtyCanvas?.(true, true);
    } finally {
      graph?.afterChange?.();
      this.#writing = false;
    }
  }
}
