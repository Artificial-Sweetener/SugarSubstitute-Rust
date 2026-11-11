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
/** Translate prompt editors' live nodes to the workflow paths regional hover links by. */
import type { ComfyNode } from '../../types/graph.js';
import type { WorkflowRegionalHover } from '../application/WorkflowRegionalHover.js';
import type { RegionalAccent } from '../application/WorkflowRegionalPrompts.js';
import type { ComfyInputAssociations } from './ComfyInputAssociations.js';

/** Prompt editors that show a linked highlight on the separators of their nodes. */
export interface RegionalPromptEditors {
  /** Highlight region `ordinal` in the editors of these prompt nodes, or clear it with null. */
  highlightRegions(prompts: ReadonlySet<ComfyNode>, ordinal: number | null): void;
}

/**
 * Address prompt editors by node while the regional hover owner addresses prompts by path, and
 * give them the accent that mask rows and canvas masks colour regions from.
 */
export class ComfyRegionalPromptLinks {
  /** Resolve paths through the association inventory that read the regional topology. */
  constructor(
    private readonly associations: Pick<ComfyInputAssociations, 'inventory'>,
    private readonly hover: Pick<WorkflowRegionalHover, 'hoverPrompt' | 'connect'>,
    readonly accent: RegionalAccent,
  ) {}

  /** Report a prompt editor's separator hover, or its end with null. */
  hoverPrompt(node: ComfyNode, ordinal: number | null): void {
    const path = this.associations.inventory.path(node);
    if (path !== null) this.hover.hoverPrompt(path, ordinal);
  }

  /** Let prompt editors show the separator of a region hovered in another view. */
  connectPrompts(editors: RegionalPromptEditors): () => void {
    return this.hover.connect({
      highlight: (prompts, ordinal) => {
        const nodes = new Set<ComfyNode>();
        for (const path of prompts) {
          const node = this.associations.inventory.node(path);
          if (node) nodes.add(node);
        }
        editors.highlightRegions(nodes, ordinal);
      },
    });
  }
}
