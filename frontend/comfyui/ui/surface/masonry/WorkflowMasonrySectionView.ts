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
/** Compose existing Cube card presentation within one continuous workflow section. */
import type { CubeNode } from '../../cube/node/ComfyCubeNodeFactory.js';
import type { CubeFaceCardStateService } from '../CubeFaceCardStateService.js';
import { NativeNodeCardHost } from '../NativeNodeCardHost.js';
import type { NativeNodeCardRenderer } from '../NativeNodeCardRenderer.js';
import type { WorkflowMasonrySection } from './WorkflowMasonryProjection.js';
import type { ComfyNode } from '../../types/graph.js';
import type { SurfaceMotion } from '../motion/SurfaceMotion.js';
import { WorkflowMasonrySectionHeaderView } from './WorkflowMasonrySectionHeaderView.js';

interface MountedMasonryCard {
  cell: HTMLElement;
  host: NativeNodeCardHost;
  node: ComfyNode;
  activation: boolean;
  prompt: boolean;
  mounted: boolean;
  retiring: boolean;
}

export interface WorkflowMasonrySectionViewOptions {
  content: HTMLElement;
  cardState: CubeFaceCardStateService;
  motion: SurfaceMotion;
  openCube(node: CubeNode): void;
}

/** Retain the shared optional-node menu while reconciling native card membership. */
export class WorkflowMasonrySectionView {
  readonly header: HTMLElement;
  readonly cells = new Map<string, HTMLElement>();
  readonly #options: WorkflowMasonrySectionViewOptions;
  readonly #headerView: WorkflowMasonrySectionHeaderView;
  readonly #cards = new Map<string, MountedMasonryCard>();

  /** Build stable section chrome using the existing optional-node toolbox presenter. */
  constructor(section: WorkflowMasonrySection, options: WorkflowMasonrySectionViewOptions) {
    this.#options = options;
    this.#headerView = new WorkflowMasonrySectionHeaderView(section, {
      document: options.content.ownerDocument,
      cardState: options.cardState,
      openCube: options.openCube,
    });
    this.header = this.#headerView.element;
    options.content.append(this.header);
  }

  /** Reconcile shared policy decisions without replacing the live menu or untouched controls. */
  prepare(section: WorkflowMasonrySection, renderer: NativeNodeCardRenderer): Array<() => void> {
    this.#headerView.render(section);
    const visible = section.cards.filter((card) => card.visible);
    const ids = new Set(visible.map((card) => card.id));
    for (const [id, record] of this.#cards) {
      const next = visible.find((card) => card.id === id);
      if (!ids.has(id)) {
        this.cells.delete(id);
        if (!record.retiring) {
          record.retiring = true;
          record.cell.style.pointerEvents = 'none';
          this.#options.motion.retire(record.cell, () => {
            releaseCard(record);
            this.#cards.delete(id);
          });
        }
      } else if (
        next?.node !== record.node ||
        next.showActivationControl !== record.activation ||
        (next.prompt === true) !== record.prompt
      ) {
        this.#options.motion.forget(record.cell);
        releaseCard(record);
        this.#cards.delete(id);
        this.cells.delete(id);
      }
    }
    const pending: Array<() => void> = [];
    for (const card of visible) {
      let record = this.#cards.get(card.id);
      if (!record) {
        const cell = this.header.ownerDocument.createElement('article');
        cell.className = 'sugarcubes-workflow-masonry__card';
        cell.style.position = 'absolute';
        cell.dataset.masonryCard = card.id;
        cell.dataset.masonrySection = section.id;
        this.#options.content.append(cell);
        this.cells.set(card.id, cell);
        record = {
          cell,
          host: new NativeNodeCardHost(renderer),
          node: card.node,
          activation: card.showActivationControl,
          prompt: card.prompt === true,
          mounted: false,
          retiring: false,
        };
        this.#cards.set(card.id, record);
      }
      if (record.retiring) {
        record.retiring = false;
        record.cell.style.removeProperty('pointer-events');
        this.cells.set(card.id, record.cell);
        this.#options.motion.revive(record.cell);
      }
      if (record.mounted) {
        record.cell.dataset.cubeColumnSpan = String(card.columnSpan);
        record.host.refresh([card]);
      } else {
        const mount = record;
        pending.push(() => {
          if (this.#cards.get(card.id) !== mount || mount.mounted) return;
          mount.host.mountCard(
            mount.cell,
            card,
            (node, enabled) =>
              this.#options.cardState.setEnabled(section.target, String(node.id ?? ''), enabled),
            { allowCollapse: card.prompt !== true },
          );
          mount.mounted = true;
        });
      }
    }
    return pending;
  }

  /** Release all native leases and section chrome when its workflow membership ends. */
  dispose(): void {
    for (const record of this.#cards.values()) {
      this.#options.motion.forget(record.cell);
      releaseCard(record);
    }
    this.#cards.clear();
    this.cells.clear();
    this.#headerView.dispose();
  }
}

/** Release one card without disturbing surviving native control instances. */
function releaseCard(record: MountedMasonryCard): void {
  record.host.dispose();
  record.cell.remove();
}
