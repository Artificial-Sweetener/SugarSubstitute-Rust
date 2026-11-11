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
/** Publish one transient regional hover to every view of the same region. */
import type { WorkflowMaskAppearance } from './WorkflowMaskAppearance.js';
import type { WorkflowRegionalPrompts } from './WorkflowRegionalPrompts.js';

/** Prompt editors that can show a linked separator highlight. */
export interface RegionalPromptHighlight {
  /** Highlight region `ordinal` in the prompts at these workflow paths, or clear it with null. */
  highlight(prompts: readonly string[], ordinal: number | null): void;
}

/** The region currently shown as hovered in every linked view. */
interface ShownHover {
  readonly endpoint: string;
  /** The hovered region's entry, or null when its prompt declares more regions than masks. */
  readonly entry: string | null;
  readonly prompts: readonly string[];
  readonly ordinal: number;
}

/**
 * Link a mask row, its canvas mask and the related prompt separators while one of them is hovered.
 *
 * Exactly one hover source is current. A clear from a source that is no longer current is
 * ignored, so moving the pointer from one view onto another never clears the newer hover. A
 * prompt related to several ordered loaders links to the first of them, in graph order. A hover
 * whose loader leaves the workflow, as when another workflow loads, is cleared.
 */
export class WorkflowRegionalHover {
  readonly #highlights = new Set<RegionalPromptHighlight>();
  readonly #listeners = new Set<() => void>();
  readonly #stop: () => void;
  #source: string | null = null;
  #shown: ShownHover | null = null;

  /** Resolve relations through the regional owner and canvas highlight through appearance. */
  constructor(
    private readonly regions: Pick<WorkflowRegionalPrompts, 'mask' | 'forPrompt' | 'subscribe'>,
    private readonly canvas: Pick<WorkflowMaskAppearance, 'hover'>,
    private readonly report: (error: unknown) => void,
  ) {
    this.#stop = regions.subscribe(() => {
      if (this.#shown && !regions.mask(this.#shown.endpoint)) this.clear();
    });
  }

  /** Hover a mask row's region, or clear the row's hover with a null entry. */
  hoverMask(endpoint: string, entry: string | null): void {
    const source = 'mask:' + endpoint;
    if (!this.#accept(source, entry)) return;
    const mask = this.regions.mask(endpoint),
      ordinal = mask && entry !== null ? mask.entries.indexOf(entry) : -1;
    this.#show({
      endpoint,
      entry,
      prompts: mask && ordinal >= 0 ? mask.prompts : [],
      ordinal,
    });
  }

  /** Hover separator `ordinal` of the prompt at this workflow path, or clear it with null. */
  hoverPrompt(prompt: string, ordinal: number | null): void {
    const source = 'prompt:' + prompt;
    if (!this.#accept(source, ordinal)) return;
    const mask = this.regions.forPrompt(prompt)[0];
    if (!mask || ordinal === null) {
      this.clear();
      return;
    }
    this.#show({
      endpoint: mask.endpoint,
      entry: mask.entries[ordinal] ?? null,
      prompts: mask.prompts,
      ordinal,
    });
  }

  /** The entry shown as hovered among this loader's rows, or null. */
  hovered(endpoint: string): string | null {
    return this.#shown?.endpoint === endpoint ? this.#shown.entry : null;
  }

  /** Clear the current hover from every linked view. */
  clear(): void {
    this.#source = null;
    const shown = this.#shown;
    if (!shown) return;
    this.#shown = null;
    this.canvas.hover(shown.endpoint, null);
    this.#highlight(shown.prompts, null);
    this.#changed();
  }

  /** Attach prompt editors; they first show the current hover, if any. */
  connect(highlight: RegionalPromptHighlight): () => void {
    this.#highlights.add(highlight);
    const shown = this.#shown;
    if (shown?.prompts.length) highlight.highlight(shown.prompts, shown.ordinal);
    return () => {
      this.#highlights.delete(highlight);
    };
  }

  /** Clear the hover and stop following regional relations. */
  dispose(): void {
    this.clear();
    this.#stop();
    this.#highlights.clear();
    this.#listeners.clear();
  }

  /** Observe changes to which rows are shown as hovered. */
  subscribe(listener: () => void): () => void {
    this.#listeners.add(listener);
    return () => {
      this.#listeners.delete(listener);
    };
  }

  /** Make a hover source current, or clear for its own null; reject stale clears. */
  #accept(source: string, value: unknown): boolean {
    if (value === null) {
      if (this.#source === source) this.clear();
      return false;
    }
    this.#source = source;
    return true;
  }

  /** Replace the shown hover in every linked view. */
  #show(next: ShownHover): void {
    const previous = this.#shown;
    if (
      previous?.endpoint === next.endpoint &&
      previous.entry === next.entry &&
      previous.ordinal === next.ordinal &&
      previous.prompts === next.prompts
    )
      return;
    if (previous) {
      if (previous.endpoint !== next.endpoint) this.canvas.hover(previous.endpoint, null);
      this.#highlight(previous.prompts, null);
    }
    this.#shown = next;
    this.canvas.hover(next.endpoint, next.entry);
    this.#highlight(next.prompts, next.ordinal);
    this.#changed();
  }

  /** Highlight prompts through every attached editor host, isolating their failures. */
  #highlight(prompts: readonly string[], ordinal: number | null): void {
    if (!prompts.length) return;
    for (const highlight of this.#highlights)
      try {
        highlight.highlight(prompts, ordinal);
      } catch (error: unknown) {
        this.report(error);
      }
  }

  /** Notify row observers, isolating their failures. */
  #changed(): void {
    for (const listener of this.#listeners)
      try {
        listener();
      } catch (error: unknown) {
        this.report(error);
      }
  }
}
