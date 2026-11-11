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
/** Retain mounted relation surfaces separately from the workflow's field inventory. */
import type { ComfyNode } from '../../types/graph.js';
import type { RelationCardSurface } from '../presentation/RelationCardSurface.js';
import { nativeRelationCards } from './NativeRelationCardAdapter.js';
import { readNativeNodeCard } from '../../surface/NativeNodeCardBoundary.js';

/** Cache native component discovery until that exact surface's contents or lifetime changes. */
export class NativeRelationViews {
  readonly #views = new Map<ComfyNode, Map<string | null, readonly RelationCardSurface[]>>();

  /** Bind discovery to the document that owns mounted native components. */
  constructor(private readonly document: Document) {}

  /** Resolve one face's retained views without inspecting unrelated Vue components. */
  read(node: ComfyNode, cubeId: string | null): readonly RelationCardSurface[] {
    let instances = this.#views.get(node);
    const cached = instances?.get(cubeId);
    if (
      cached?.length &&
      cached.every((surface) => {
        const boundary = readNativeNodeCard(surface.root);
        return (
          surface.root.isConnected &&
          !surface.canvasTitle &&
          (cubeId === null || (boundary?.node === node && boundary.cubeId === cubeId))
        );
      })
    )
      return cached;
    const surfaces = nativeRelationCards(this.document, node, cubeId);
    if (!instances) this.#views.set(node, (instances = new Map()));
    instances.set(cubeId, surfaces);
    return surfaces;
  }

  /** Invalidate only cached views touched by identified native surface changes. */
  invalidate(elements: readonly HTMLElement[]): void {
    for (const [node, instances] of this.#views) {
      for (const [cubeId, surfaces] of instances) {
        if (
          !surfaces.length ||
          elements.some((element) => {
            const boundary = readNativeNodeCard(element);
            return (
              (boundary?.node === node && boundary.cubeId === cubeId) ||
              (cubeId === null && element.dataset.nodeId === String(node.id))
            );
          }) ||
          surfaces.some(
            (surface) =>
              !surface.root.isConnected ||
              elements.some(
                (element) => element.contains(surface.root) || surface.root.contains(element),
              ),
          )
        )
          instances.delete(cubeId);
      }
      if (!instances.size) this.#views.delete(node);
    }
  }

  /** Drop detached component references at workflow replacement. */
  clear(): void {
    this.#views.clear();
  }
}
