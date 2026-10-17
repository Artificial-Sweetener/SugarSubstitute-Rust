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
/** Coordinate reversible native-card layout assembly and graph chrome fades. */
import { fluentMotion } from '../appearance/FluentThemeRuntime.js';
import { NativeGraphCardAssembly } from './NativeGraphCardAssembly.js';
import { setSurfaceGeometryMoving } from '../motion/SurfaceGeometryMotion.js';
import type { WorkflowMasonryGraphReveal } from './WorkflowMasonryGraphReveal.js';
import {
  rectanglesIntersect,
  type WorkflowMasonryGraphSnapshot,
} from './WorkflowMasonryGraphGeometry.js';

export interface WorkflowMasonryTransitionCard {
  id: string;
  cell: HTMLElement;
}

export interface WorkflowMasonryTransitionSurface {
  surface: HTMLElement;
  backdrop: HTMLElement;
  cards: readonly WorkflowMasonryTransitionCard[];
  measureGraph(): WorkflowMasonryGraphSnapshot;
  reveal?: WorkflowMasonryGraphReveal;
}

interface MotionRequest {
  element: HTMLElement;
  frames: Keyframe[];
  source?: HTMLElement;
}

/** Own all temporary effects and source suppression without retaining either presentation. */
export class WorkflowMasonryTransition {
  readonly #effects = new Map<HTMLElement, Animation>();
  readonly #marked = new Set<HTMLElement>();
  readonly #sources = new Set<HTMLElement>();
  readonly #graphAssemblies = new Map<HTMLElement, NativeGraphCardAssembly>();
  #surface: HTMLElement | null = null;
  #surfaceInert = false;
  #generation = 0;

  /** Measure endpoints before writes and interpolate native width independently of graph zoom. */
  async run(
    view: WorkflowMasonryTransitionSurface,
    entering: boolean,
    reducedMotion: boolean,
  ): Promise<void> {
    const windowRef = view.surface.ownerDocument.defaultView;
    const requests: MotionRequest[] = [];
    const opacityFrames = (element: HTMLElement, from: number, to: number): Keyframe[] => [
      {
        opacity: this.#effects.has(element)
          ? (windowRef?.getComputedStyle(element).opacity ?? String(from))
          : from,
      },
      { opacity: to },
    ];
    const graph = reducedMotion ? null : view.measureGraph();
    const reveal = view.reveal?.frames(entering) ?? [];
    if (graph) {
      requests.push(...reveal);
      requests.push({
        element: view.backdrop,
        frames: opacityFrames(view.backdrop, entering ? 0 : 1, entering ? 1 : 0),
      });
      for (const layer of graph.layers)
        requests.push({
          element: layer,
          frames: opacityFrames(layer, entering ? 1 : 0, entering ? 0 : 1),
        });
      for (const header of view.surface.querySelectorAll<HTMLElement>(
        '.sugarcubes-workflow-masonry__section',
      ))
        requests.push({
          element: header,
          frames: opacityFrames(header, entering ? 0 : 1, entering ? 1 : 0),
        });
      for (const card of view.cards) {
        const destination = card.cell.getBoundingClientRect();
        if (destination.width <= 0 || destination.height <= 0) continue;
        const measured = graph.cards.get(card.id);
        let graphAssembly = measured ? this.#graphAssemblies.get(measured.element) : undefined;
        const source = graphAssembly?.source ?? measured;
        if (!source?.visible && !rectanglesIntersect(destination, graph.viewport)) continue;
        if (!source) {
          requests.push({
            element: card.cell,
            frames: opacityFrames(card.cell, entering ? 0 : 1, entering ? 1 : 0),
          });
          continue;
        }
        if (source.element.matches('.lg-node') && typeof source.element.animate === 'function') {
          graphAssembly ??= new NativeGraphCardAssembly(source);
          this.#graphAssemblies.set(source.element, graphAssembly);
          requests.push({
            element: source.element,
            frames: graphAssembly.frames(destination, entering, this.#effects.has(source.element)),
          });
        }
        const rectangle = source.rectangle;
        const placement = card.cell.style.transform;
        const masonryTransform = placement || 'translate(0px, 0px) scale(1)';
        const graphTransform = `${placement ? `${placement} ` : ''}translate(${String(rectangle.x - destination.x)}px, ${String(rectangle.y - destination.y)}px) scale(${String(source.scale)})`;
        const current = this.#effects.has(card.cell)
          ? windowRef?.getComputedStyle(card.cell)
          : undefined;
        const graphWidth = `${String(source.layoutWidth)}px`;
        const masonryWidth = `${String(destination.width)}px`;
        const graphHeight = `${String(rectangle.height / source.scale)}px`;
        const masonryHeight = `${String(destination.height)}px`;
        requests.push({
          element: card.cell,
          ...(graphAssembly ? {} : { source: source.element }),
          frames: [
            {
              transform:
                current?.transform && current.transform !== 'none'
                  ? current.transform
                  : entering
                    ? graphTransform
                    : masonryTransform,
              opacity: graphAssembly ? current?.opacity || (entering ? 0 : 1) : 1,
              width: current?.width || (entering ? graphWidth : masonryWidth),
              ...(graphAssembly
                ? { height: current?.height || (entering ? graphHeight : masonryHeight) }
                : {}),
            },
            {
              transform: entering ? masonryTransform : graphTransform,
              opacity: graphAssembly ? (entering ? 1 : 0) : 1,
              width: entering ? masonryWidth : graphWidth,
              ...(graphAssembly ? { height: entering ? masonryHeight : graphHeight } : {}),
            },
          ],
        });
      }
    }
    this.#cancelEffects();
    const generation = this.#generation;
    if (view.surface.style.opacity !== '1') view.surface.style.opacity = '1';
    delete view.surface.dataset.scMasonryPreparing;
    const backdropOpacity = entering ? '1' : '0';
    if (view.backdrop.style.opacity !== backdropOpacity)
      view.backdrop.style.opacity = backdropOpacity;
    if (requests.length === 0) {
      this.cancel();
      return;
    }
    this.#surface = view.surface;
    this.#surfaceInert = view.surface.inert;
    view.surface.inert = true;
    view.surface.dataset.scMasonryTransition = entering ? 'enter' : 'exit';
    const completions: Array<Promise<PromiseSettledResult<Animation>[]>> = [];
    try {
      setSurfaceGeometryMoving(view.surface, true);
      for (const assembly of this.#graphAssemblies.values()) assembly.attach();
      for (const request of requests) {
        if (typeof request.element.animate !== 'function') continue;
        request.element.dataset.scMasonryMoving = '';
        if (this.#graphAssemblies.has(request.element))
          setSurfaceGeometryMoving(request.element, true);
        this.#marked.add(request.element);
        if (request.source) {
          request.source.dataset.scMasonrySource = '';
          this.#sources.add(request.source);
        }
      }
      for (const request of requests) {
        if (typeof request.element.animate !== 'function') continue;
        const effect = request.element.animate(request.frames, {
          duration: fluentMotion.duration,
          easing: fluentMotion.easing,
          fill: 'both',
        });
        this.#effects.set(request.element, effect);
        completions.push(Promise.allSettled([effect.finished]));
      }
      await Promise.all(completions);
    } finally {
      if (generation === this.#generation) this.cancel();
    }
  }

  /** Restore source visibility and release all compositor effects on every teardown path. */
  cancel(): void {
    this.#cancelEffects();
    for (const assembly of this.#graphAssemblies.values()) assembly.dispose();
    this.#graphAssemblies.clear();
  }

  /** Stop old effects during reversal while retaining their exact original graph endpoints. */
  #cancelEffects(): void {
    this.#generation += 1;
    for (const effect of this.#effects.values()) {
      effect.cancel();
    }
    this.#effects.clear();
    for (const element of this.#marked) {
      delete element.dataset.scMasonryMoving;
      if (this.#graphAssemblies.has(element)) setSurfaceGeometryMoving(element, false);
    }
    this.#marked.clear();
    for (const source of this.#sources) delete source.dataset.scMasonrySource;
    this.#sources.clear();
    if (this.#surface) {
      setSurfaceGeometryMoving(this.#surface, false);
      delete this.#surface.dataset.scMasonryTransition;
      this.#surface.inert = this.#surfaceInert;
    }
    this.#surface = null;
  }
}
