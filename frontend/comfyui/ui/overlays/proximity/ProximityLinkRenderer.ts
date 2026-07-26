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
/** Render transient dotted links through Comfy's native link primitive. */

import type { UnknownRecord } from '../../types/common.js';
import type { ComfyGraph, ComfyInput, ComfyOutput } from '../../types/graph.js';
import type { ComfyNode } from '../../types/graph.js';
import type { Vec2 } from '../../types/common.js';
import { ComfyGraphGeometry } from './ComfyGraphGeometry.js';
import type { ProximityLiteGraph } from './ProximityMatcher.js';
import type { ProximityMatch } from './ProximityModel.js';

type LinkSlot = (ComfyInput | ComfyOutput) & UnknownRecord;

export interface ProximityRenderCanvas extends UnknownRecord {
  graph?: ComfyGraph;
  ds?: { scale?: number };
  connections_width?: number;
  default_link_color?: string;
  linkMarkerShape?: unknown;
  renderLink?: (...args: unknown[]) => unknown;
  getLinkColor?(slotType: unknown): unknown;
}

export interface ProximityLinkRendererOptions {
  getLiteGraph(): ProximityLiteGraph | null;
  logger: Pick<Console, 'debug' | 'warn'>;
}

export interface ProximityPortPositionSource {
  resolveGraphPosition(
    node: ComfyNode,
    direction: 'input' | 'output',
    index: number,
    fallback: Vec2,
  ): Vec2;
}

/** Own dotted-guide styling and live native-slot geometry. */
export class ProximityLinkRenderer {
  readonly #getLiteGraph: () => ProximityLiteGraph | null;
  readonly #logger: Pick<Console, 'debug' | 'warn'>;
  readonly #geometry: ComfyGraphGeometry;
  #positionSource: ProximityPortPositionSource | null = null;
  #lastReportedRenderSignature = '';

  /** Bind Comfy's renderer capabilities without owning match policy. */
  constructor(options: ProximityLinkRendererOptions) {
    this.#getLiteGraph = options.getLiteGraph;
    this.#logger = options.logger;
    this.#geometry = new ComfyGraphGeometry(options.logger);
  }

  /** Bind the graph-scoped owner of animated Cube socket positions. */
  setPositionSource(source: ProximityPortPositionSource | null): void {
    this.#positionSource = source;
  }

  /** Draw the authoritative matches from their current animated hit targets. */
  render(
    matches: readonly ProximityMatch[],
    context: CanvasRenderingContext2D,
    canvas: ProximityRenderCanvas,
  ): void {
    const liteGraph = this.#getLiteGraph();
    let graphMismatchCount = 0;
    let renderedCount = 0;
    for (const match of matches) {
      const outputNode = match.outputNode;
      const inputNode = match.inputNode;
      if (
        (outputNode && outputNode.graph !== canvas.graph) ||
        (inputNode && inputNode.graph !== canvas.graph)
      ) {
        graphMismatchCount += 1;
        continue;
      }
      const outputSlotIndex = match.outputSlot ?? 0;
      const inputSlotIndex = match.inputSlot ?? 0;
      const outputFallback = outputNode
        ? this.#geometry.slotPosition(outputNode, true, outputSlotIndex)
        : match.outputPos;
      const inputFallback = inputNode
        ? this.#geometry.slotPosition(inputNode, false, inputSlotIndex)
        : match.inputPos;
      const outputPosition = outputNode
        ? (this.#positionSource?.resolveGraphPosition(
            outputNode,
            'output',
            outputSlotIndex,
            outputFallback,
          ) ?? outputFallback)
        : outputFallback;
      const inputPosition = inputNode
        ? (this.#positionSource?.resolveGraphPosition(
            inputNode,
            'input',
            inputSlotIndex,
            inputFallback,
          ) ?? inputFallback)
        : inputFallback;
      const outputSlot = outputNode?.outputs?.[outputSlotIndex];
      const inputSlot = inputNode?.inputs?.[inputSlotIndex];
      const slotType = match.outputType ?? match.inputType ?? outputSlot?.type ?? inputSlot?.type;
      const color = resolveLinkColor(canvas, liteGraph, slotType, this.#logger);
      if (
        this.#drawNativeLink({
          context,
          canvas,
          start: outputPosition,
          end: inputPosition,
          outputSlot,
          inputSlot,
          slotType,
          color,
        })
      ) {
        renderedCount += 1;
      }
    }
    const signature = `${String(renderedCount)}:${String(graphMismatchCount)}:${String(matches.length)}`;
    if (signature === this.#lastReportedRenderSignature) return;
    this.#lastReportedRenderSignature = signature;
    this.#logger.debug(
      `SugarCubes painted ${String(renderedCount)} of ${String(matches.length)} proximity matches` +
        `${graphMismatchCount ? `; ${String(graphMismatchCount)} had a graph mismatch` : ''}.`,
    );
  }

  /** Render one guide with Comfy's selected native link mode and a dashed stroke. */
  #drawNativeLink(options: {
    context: CanvasRenderingContext2D;
    canvas: ProximityRenderCanvas;
    start: readonly [number, number];
    end: readonly [number, number];
    outputSlot: LinkSlot | undefined;
    inputSlot: LinkSlot | undefined;
    slotType: unknown;
    color: string;
  }): boolean {
    const renderLink =
      typeof options.canvas.renderLink === 'function'
        ? options.canvas.renderLink.bind(options.canvas)
        : null;
    if (!renderLink) return false;
    const liteGraph = this.#getLiteGraph();
    const connectionWidth = Math.max(1, Number(options.canvas.connections_width) || 3);
    const scale = Number(options.canvas.ds?.scale) || 1;
    const dashPattern = computeDashPattern(connectionWidth, scale);
    const length = Math.hypot(options.end[0] - options.start[0], options.end[1] - options.start[1]);
    const dashCycle = dashPattern[0] + dashPattern[1];
    const dashOffset = ((length * 0.5) % dashCycle) - dashPattern[0] * 0.5;
    const fakeLink = {
      id: -1,
      type: options.slotType,
      _pos: new Float32Array(2),
    };
    const previousMarkerShape = options.canvas.linkMarkerShape;
    options.canvas.linkMarkerShape = liteGraph?.LinkMarkerShape?.None ?? 0;
    options.context.save();
    try {
      options.context.strokeStyle = options.color;
      options.context.setLineDash(dashPattern);
      options.context.lineDashOffset = dashOffset;
      renderLink(
        options.context,
        [options.start[0], options.start[1]],
        [options.end[0], options.end[1]],
        fakeLink,
        false,
        false,
        null,
        resolveSlotDirection(options.outputSlot, true, liteGraph),
        resolveSlotDirection(options.inputSlot, false, liteGraph),
        { disabled: false },
      );
      return true;
    } catch (error: unknown) {
      this.#logger.warn('SugarCubes: proximity renderLink failed', error);
      return false;
    } finally {
      options.context.restore();
      options.canvas.linkMarkerShape = previousMarkerShape;
    }
  }
}

/** Resolve a scale-aware dotted pattern without tiny or giant segments. */
export function computeDashPattern(connectionWidth: number, scale: number): [number, number] {
  const safeWidth = Math.max(1, Number(connectionWidth) || 1);
  const safeScale = clamp(Number(scale) || 1, 0.2, 5);
  return [
    clamp(safeWidth * 2.8, 6 / safeScale, 48 / safeScale),
    clamp(safeWidth * 1.6, 4 / safeScale, 32 / safeScale),
  ];
}

/** Prefer native per-slot direction metadata before Comfy defaults. */
function resolveSlotDirection(
  slot: LinkSlot | undefined,
  isOutput: boolean,
  liteGraph: ProximityLiteGraph | null,
): unknown {
  if (slot?.dir !== undefined && slot.dir !== null) return slot.dir;
  return isOutput ? (liteGraph?.LinkDirection?.RIGHT ?? 4) : (liteGraph?.LinkDirection?.LEFT ?? 3);
}

/** Resolve the same type color Comfy uses for persisted links. */
function resolveLinkColor(
  canvas: ProximityRenderCanvas,
  liteGraph: ProximityLiteGraph | null,
  slotType: unknown,
  logger: Pick<Console, 'warn'>,
): string {
  if (typeof canvas.getLinkColor === 'function') {
    try {
      const color = canvas.getLinkColor(slotType);
      if (typeof color === 'string' && color) return color;
    } catch (error: unknown) {
      logger.warn('SugarCubes failed to resolve proximity-link color.', error);
    }
  }
  if (liteGraph?.EVENT !== undefined && slotType === liteGraph.EVENT) {
    return liteGraph.EVENT_LINK_COLOR || '#AFA';
  }
  return canvas.default_link_color || '#7fc4ff';
}

/** Clamp one value to an inclusive range. */
function clamp(value: number, minimum: number, maximum: number): number {
  return Math.min(Math.max(value, minimum), maximum);
}
