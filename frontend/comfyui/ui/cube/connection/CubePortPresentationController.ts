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
/** Animate transient Cube boundary ports while preserving canonical graph slots. */

import type {
  ProximityMatch,
  ProximityMatchSink,
} from '../../overlays/proximity/ProximityModel.js';
import type { Vec2 } from '../../types/common.js';
import type { ComfyNode } from '../../types/graph.js';

export type CubePortDirection = 'input' | 'output';

export interface CubePortAnchor {
  index: number;
  defaultY: number;
  minY: number;
  maxY: number;
  labelY: number;
}

export interface CubePortPresentationControllerOptions {
  now?: () => number;
  requestFrame?: (callback: FrameRequestCallback) => number | null;
  invalidate?: () => void;
  durationMs?: number;
}

interface PortTransition extends CubePortAnchor {
  currentY: number;
  fromY: number;
  targetY: number;
  startedAt: number;
}

interface NodePresentation {
  input: Map<number, PortTransition>;
  output: Map<number, PortTransition>;
  resolveOriginY: () => number;
}

interface PortAttraction {
  index: number;
  desiredY: number;
  laneY: number;
}

const DEFAULT_DURATION_MS = 180;
const MIN_PORT_SEPARATION = 18;

/**
 * Own renderer-neutral animated port positions and temporary visual ordering.
 *
 * Canonical slot indices remain the map keys. Only local presentation Y values
 * transition, so serialization and native prompt topology never observe swaps.
 */
export class CubePortPresentationController implements ProximityMatchSink {
  readonly #presentations = new Map<ComfyNode, NodePresentation>();
  readonly #now: () => number;
  readonly #requestFrame: (callback: FrameRequestCallback) => number | null;
  readonly #invalidate: () => void;
  readonly #durationMs: number;
  readonly #subscribers = new Map<ComfyNode, Set<() => void>>();
  #matches: readonly ProximityMatch[] = [];
  #framePending = false;

  /** Bind animation timing and the host repaint seam. */
  constructor(options: CubePortPresentationControllerOptions = {}) {
    this.#now = options.now ?? (() => globalThis.performance?.now?.() ?? Date.now());
    this.#requestFrame =
      options.requestFrame ?? ((callback) => globalThis.requestAnimationFrame?.(callback) ?? null);
    this.#invalidate = options.invalidate ?? (() => undefined);
    this.#durationMs = Math.max(1, options.durationMs ?? DEFAULT_DURATION_MS);
  }

  /** Register stable local anchors measured by either Comfy renderer. */
  register(
    node: ComfyNode,
    direction: CubePortDirection,
    anchors: readonly CubePortAnchor[],
  ): void {
    const presentation = this.#presentations.get(node) ?? {
      input: new Map<number, PortTransition>(),
      output: new Map<number, PortTransition>(),
      resolveOriginY: () => readNodeY(node),
    };
    this.#presentations.set(node, presentation);
    const ports = presentation[direction];
    const currentIndices = new Set(anchors.map((anchor) => anchor.index));
    let anchorsChanged = false;
    for (const index of [...ports.keys()]) {
      if (!currentIndices.has(index)) {
        ports.delete(index);
        anchorsChanged = true;
      }
    }
    const now = this.#now();
    for (const anchor of anchors) {
      const existing = ports.get(anchor.index);
      if (existing) {
        const normalized = normalizeAnchor(anchor);
        if (sameAnchor(existing, normalized)) continue;
        const currentY = interpolate(existing, now, this.#durationMs);
        Object.assign(existing, normalized, {
          currentY,
          fromY: currentY,
          targetY: normalized.defaultY,
          startedAt: now,
        });
        anchorsChanged = true;
      } else {
        const normalized = normalizeAnchor(anchor);
        ports.set(anchor.index, {
          ...normalized,
          currentY: normalized.defaultY,
          fromY: normalized.defaultY,
          targetY: normalized.defaultY,
          startedAt: now,
        });
        anchorsChanged = true;
      }
    }
    if (anchorsChanged) this.#reconcileTargets(now);
  }

  /** Bind the renderer's authoritative graph-space node origin without DOM reads per frame. */
  registerOrigin(node: ComfyNode, resolveOriginY: () => number): void {
    const existing = this.#presentations.get(node);
    if (existing?.resolveOriginY === resolveOriginY) return;
    const presentation = existing ?? {
      input: new Map<number, PortTransition>(),
      output: new Map<number, PortTransition>(),
      resolveOriginY,
    };
    presentation.resolveOriginY = resolveOriginY;
    this.#presentations.set(node, presentation);
    this.#reconcileTargets(this.#now());
  }

  /** Remove all transient geometry for a Cube that left the presentation. */
  release(node: ComfyNode): void {
    this.#presentations.delete(node);
    this.#subscribers.delete(node);
  }

  /** Subscribe one mounted renderer host only to transitions for its Cube. */
  subscribe(node: ComfyNode, listener: () => void): () => void {
    const listeners = this.#subscribers.get(node) ?? new Set<() => void>();
    this.#subscribers.set(node, listeners);
    listeners.add(listener);
    return () => {
      listeners.delete(listener);
      if (!listeners.size) this.#subscribers.delete(node);
    };
  }

  /** Replace the authoritative proximity set used by every rendered boundary. */
  updateMatches(matches: readonly ProximityMatch[]): void {
    this.#matches = [...matches];
    this.#reconcileTargets(this.#now());
  }

  /** Resolve one current local Y without mutating the canonical slot index. */
  resolveLocalY(node: ComfyNode, direction: CubePortDirection, index: number): number | null {
    const port = this.#presentations.get(node)?.[direction].get(index);
    if (!port) return null;
    const currentY = interpolate(port, this.#now(), this.#durationMs);
    port.currentY = currentY;
    return currentY;
  }

  /** Report whether one Cube still has an in-flight magnetic transition. */
  isAnimating(node: ComfyNode): boolean {
    return this.#nodeHasActiveTransition(node, this.#now());
  }

  /** Resolve stable graph geometry for matching without animation feedback. */
  resolveDefaultGraphPosition(
    node: ComfyNode,
    direction: CubePortDirection,
    index: number,
    fallback: Vec2,
  ): Vec2 {
    const port = this.#presentations.get(node)?.[direction].get(index);
    if (!port) return fallback;
    return [fallback[0], this.#resolveOriginY(node) + port.defaultY];
  }

  /** Resolve the settled target used for matching without animation feedback. */
  resolveMatchingGraphPosition(
    node: ComfyNode,
    direction: CubePortDirection,
    index: number,
    fallback: Vec2,
  ): Vec2 {
    const port = this.#presentations.get(node)?.[direction].get(index);
    if (!port) return fallback;
    return [fallback[0], this.#resolveOriginY(node) + port.targetY];
  }

  /** Resolve the authoritative rendered graph position for guides and hit targets. */
  resolveGraphPosition(
    node: ComfyNode,
    direction: CubePortDirection,
    index: number,
    fallback: Vec2,
  ): Vec2 {
    const localY = this.resolveLocalY(node, direction, index);
    return localY === null ? fallback : [fallback[0], this.#resolveOriginY(node) + localY];
  }

  /** Return no persisted state because magnetic presentation is intentionally ephemeral. */
  serialize(): Record<string, never> {
    return {};
  }

  /** Recompute visual lanes from stable anchors and the current match set. */
  #reconcileTargets(now: number): void {
    const attractions = collectAttractions(this.#matches, this.#presentations);
    let changed = false;
    const changedNodes = new Set<ComfyNode>();
    for (const [node, presentation] of this.#presentations) {
      for (const direction of ['input', 'output'] as const) {
        const ports = [...presentation[direction].values()];
        const targets = resolvePortTargets(
          ports,
          attractions.get(node)?.[direction] ?? [],
          resolvePresentationOrigin(node, presentation),
          direction,
        );
        for (const port of ports) {
          const currentY = interpolate(port, now, this.#durationMs);
          const targetY = targets.get(port.index) ?? port.defaultY;
          if (Math.abs(targetY - port.targetY) < 0.01) continue;
          port.currentY = currentY;
          port.fromY = currentY;
          port.targetY = targetY;
          port.startedAt = now;
          changed = true;
          changedNodes.add(node);
        }
      }
    }
    if (!changed) {
      if (this.#hasActiveTransition(now)) this.#scheduleFrame();
      return;
    }
    this.#invalidate();
    for (const node of changedNodes) this.#notify(node);
    this.#scheduleFrame();
  }

  /** Resolve the current renderer-owned graph origin with a native-node fallback. */
  #resolveOriginY(node: ComfyNode): number {
    const presentation = this.#presentations.get(node);
    return presentation ? resolvePresentationOrigin(node, presentation) : readNodeY(node);
  }

  /** Drive repaints only while at least one transition remains active. */
  #scheduleFrame(): void {
    if (this.#framePending) return;
    this.#framePending = true;
    const scheduled = this.#requestFrame(() => {
      this.#framePending = false;
      const now = this.#now();
      this.#invalidate();
      for (const node of this.#presentations.keys()) {
        if (this.#advanceNode(node, now)) this.#notify(node);
      }
      if (this.#hasActiveTransition(now)) this.#scheduleFrame();
    });
    if (scheduled === null) this.#framePending = false;
  }

  /** Check active transitions without allocating draw-loop state. */
  #hasActiveTransition(now: number): boolean {
    for (const node of this.#presentations.keys()) {
      if (this.#nodeHasActiveTransition(node, now)) return true;
    }
    return false;
  }

  /** Check one Cube without waking unrelated DOM boundary hosts. */
  #nodeHasActiveTransition(node: ComfyNode, now: number): boolean {
    const presentation = this.#presentations.get(node);
    if (!presentation) return false;
    for (const direction of ['input', 'output'] as const) {
      for (const port of presentation[direction].values()) {
        if (
          Math.abs(port.targetY - port.fromY) >= 0.01 &&
          now - port.startedAt < this.#durationMs
        ) {
          return true;
        }
      }
    }
    return false;
  }

  /** Advance current values and emit one final exact frame when motion settles. */
  #advanceNode(node: ComfyNode, now: number): boolean {
    const presentation = this.#presentations.get(node);
    if (!presentation) return false;
    let changed = false;
    for (const direction of ['input', 'output'] as const) {
      for (const port of presentation[direction].values()) {
        if (Math.abs(port.targetY - port.fromY) < 0.01) continue;
        const nextY = interpolate(port, now, this.#durationMs);
        changed = changed || Math.abs(nextY - port.currentY) >= 0.01;
        port.currentY = nextY;
        if (now - port.startedAt >= this.#durationMs) {
          port.currentY = port.targetY;
          port.fromY = port.targetY;
          port.startedAt = now;
          changed = true;
        }
      }
    }
    return changed;
  }

  /** Notify one mounted renderer boundary after its transient state changes. */
  #notify(node: ComfyNode): void {
    for (const listener of this.#subscribers.get(node) ?? []) listener();
  }
}

/** Collect attractions for every presented Cube endpoint in an authoritative match. */
function collectAttractions(
  matches: readonly ProximityMatch[],
  presentations: ReadonlyMap<ComfyNode, NodePresentation>,
): Map<ComfyNode, { input: PortAttraction[]; output: PortAttraction[] }> {
  const result = new Map<ComfyNode, { input: PortAttraction[]; output: PortAttraction[] }>();
  for (const match of matches) {
    if (!match.outputNode || !match.inputNode) continue;
    const outputPresentation = presentations.get(match.outputNode);
    const inputPresentation = presentations.get(match.inputNode);
    if (!outputPresentation && !inputPresentation) continue;
    if (!outputPresentation || !inputPresentation) {
      if (outputPresentation && match.outputCube != null) {
        pushAttraction(
          result,
          match.outputNode,
          'output',
          match.outputSlot,
          match.inputPos[1],
          match.inputPos[1],
        );
      }
      if (inputPresentation && match.inputCube != null) {
        pushAttraction(
          result,
          match.inputNode,
          'input',
          match.inputSlot,
          match.outputPos[1],
          match.outputPos[1],
        );
      }
      continue;
    }
    const outputDefault = resolveDefaultGraphY(
      presentations,
      match.outputNode,
      'output',
      match.outputSlot,
      match.outputPos[1],
    );
    const inputDefault = resolveDefaultGraphY(
      presentations,
      match.inputNode,
      'input',
      match.inputSlot,
      match.inputPos[1],
    );
    const preferredY =
      match.outputSlot < match.inputSlot
        ? outputDefault
        : match.inputSlot < match.outputSlot
          ? inputDefault
          : (outputDefault + inputDefault) / 2;
    const outputPort = presentations.get(match.outputNode)?.output.get(match.outputSlot);
    const inputPort = presentations.get(match.inputNode)?.input.get(match.inputSlot);
    const sharedY = resolveFeasibleMatchY(
      preferredY,
      resolvePresentationOrigin(match.outputNode, outputPresentation),
      outputPort,
      resolvePresentationOrigin(match.inputNode, inputPresentation),
      inputPort,
    );
    pushAttraction(result, match.outputNode, 'output', match.outputSlot, sharedY, preferredY);
    pushAttraction(result, match.inputNode, 'input', match.inputSlot, sharedY, preferredY);
  }
  return result;
}

/** Add one desired graph Y to a node-direction collection. */
function pushAttraction(
  target: Map<ComfyNode, { input: PortAttraction[]; output: PortAttraction[] }>,
  node: ComfyNode,
  direction: CubePortDirection,
  index: number,
  desiredY: number,
  laneY: number,
): void {
  const nodeAttractions = target.get(node) ?? { input: [], output: [] };
  target.set(node, nodeAttractions);
  nodeAttractions[direction].push({ index, desiredY, laneY });
}

/** Resolve one registered default graph Y with the endpoint fallback as a safety net. */
function resolveDefaultGraphY(
  presentations: ReadonlyMap<ComfyNode, NodePresentation>,
  node: ComfyNode,
  direction: CubePortDirection,
  index: number,
  fallback: number,
): number {
  const port = presentations.get(node)?.[direction].get(index);
  const presentation = presentations.get(node);
  return port && presentation
    ? resolvePresentationOrigin(node, presentation) + port.defaultY
    : fallback;
}

/** Assign attracted slots to visual lanes while leaving unmatched slots in remaining lanes. */
function resolvePortTargets(
  ports: readonly PortTransition[],
  attractions: readonly PortAttraction[],
  nodeY: number,
  direction: CubePortDirection,
): Map<number, number> {
  const targets = new Map(ports.map((port) => [port.index, port.defaultY]));
  if (!ports.length || !attractions.length) return targets;
  const orderedLanes = [...ports].sort(
    (left, right) => left.defaultY - right.defaultY || left.index - right.index,
  );
  const attractionByIndex = new Map(
    attractions.map((attraction) => [attraction.index, attraction.desiredY - nodeY]),
  );
  const laneAttractionByIndex = new Map(
    attractions.map((attraction) => [attraction.index, attraction.laneY - nodeY]),
  );
  const attracted = ports
    .filter((port) => attractionByIndex.has(port.index))
    .sort((left, right) => {
      const leftY = laneAttractionByIndex.get(left.index) ?? left.defaultY;
      const rightY = laneAttractionByIndex.get(right.index) ?? right.defaultY;
      return leftY - rightY || left.index - right.index;
    });
  const freeLanes = [...orderedLanes];
  const assigned = new Map<number, PortTransition>();
  for (const port of attracted) {
    const desiredY = laneAttractionByIndex.get(port.index) ?? port.defaultY;
    const laneIndex = nearestLaneIndex(freeLanes, desiredY);
    const [lane] = freeLanes.splice(laneIndex, 1);
    if (lane) assigned.set(port.index, lane);
  }
  const unmatched = ports
    .filter((port) => !assigned.has(port.index))
    .sort((left, right) => left.index - right.index);
  for (const [index, port] of unmatched.entries()) {
    const lane = freeLanes[index];
    if (lane) assigned.set(port.index, lane);
  }
  const visualOrder = [...ports].sort((left, right) => {
    const leftLane = assigned.get(left.index)?.defaultY ?? left.defaultY;
    const rightLane = assigned.get(right.index)?.defaultY ?? right.defaultY;
    return leftLane - rightLane || left.index - right.index;
  });
  const separation = resolveFeasibleSeparation(visualOrder);
  const upperBounds = resolveOrderedUpperBounds(visualOrder, separation);
  let previousY = Number.NEGATIVE_INFINITY;
  for (const [index, port] of visualOrder.entries()) {
    const desired = attractionByIndex.get(port.index);
    const lane = assigned.get(port.index);
    const rawTarget = desired ?? lane?.defaultY ?? port.defaultY;
    const lowerBound = Math.max(
      port.minY,
      direction === 'output' ? port.labelY : port.minY,
      previousY + separation,
    );
    const target = clamp(rawTarget, lowerBound, upperBounds[index] ?? port.maxY);
    targets.set(port.index, target);
    previousY = target;
  }
  return targets;
}

/** Keep a matched pair on one graph Y that both endpoint presentations can reach. */
function resolveFeasibleMatchY(
  preferredY: number,
  outputNodeY: number,
  output: PortTransition | undefined,
  inputNodeY: number,
  input: PortTransition | undefined,
): number {
  if (!output || !input) return preferredY;
  const minimum = Math.max(outputNodeY + output.labelY, inputNodeY + input.minY);
  const maximum = Math.min(outputNodeY + output.maxY, inputNodeY + input.maxY);
  return minimum <= maximum ? clamp(preferredY, minimum, maximum) : preferredY;
}

/** Reduce separation only when the registered travel range cannot hold every port. */
function resolveFeasibleSeparation(ports: readonly PortTransition[]): number {
  let separation = MIN_PORT_SEPARATION;
  for (let left = 0; left < ports.length; left += 1) {
    for (let right = left + 1; right < ports.length; right += 1) {
      const leftPort = ports[left];
      const rightPort = ports[right];
      if (!leftPort || !rightPort) continue;
      const available = (rightPort.maxY - leftPort.minY) / (right - left);
      separation = Math.min(separation, Math.max(0, available));
    }
  }
  return separation;
}

/** Reserve enough upper space for every later port before assigning current targets. */
function resolveOrderedUpperBounds(ports: readonly PortTransition[], separation: number): number[] {
  const upperBounds = ports.map((port) => port.maxY);
  for (let index = ports.length - 2; index >= 0; index -= 1) {
    const port = ports[index];
    const nextUpper = upperBounds[index + 1];
    if (!port || nextUpper === undefined) continue;
    upperBounds[index] = Math.max(port.minY, Math.min(port.maxY, nextUpper - separation));
  }
  return upperBounds;
}

/** Select the available visual lane closest to one desired local position. */
function nearestLaneIndex(lanes: readonly PortTransition[], desiredY: number): number {
  let selected = 0;
  let distance = Number.POSITIVE_INFINITY;
  for (const [index, lane] of lanes.entries()) {
    const candidateDistance = Math.abs(lane.defaultY - desiredY);
    if (candidateDistance < distance) {
      distance = candidateDistance;
      selected = index;
    }
  }
  return selected;
}

/** Interpolate one transition with a smooth finite ease curve. */
function interpolate(port: PortTransition, now: number, durationMs: number): number {
  const progress = clamp((now - port.startedAt) / durationMs, 0, 1);
  const eased = progress * progress * (3 - 2 * progress);
  return port.fromY + (port.targetY - port.fromY) * eased;
}

/** Normalize external layout geometry before it enters animation state. */
function normalizeAnchor(anchor: CubePortAnchor): CubePortAnchor {
  const minY = finite(anchor.minY, 0);
  const maxY = Math.max(minY, finite(anchor.maxY, minY));
  return {
    index: Math.max(0, Math.trunc(finite(anchor.index, 0))),
    defaultY: clamp(finite(anchor.defaultY, minY), minY, maxY),
    minY,
    maxY,
    labelY: clamp(finite(anchor.labelY, minY), minY, maxY),
  };
}

/** Avoid restarting animation when a render pass reports unchanged geometry. */
function sameAnchor(left: CubePortAnchor, right: CubePortAnchor): boolean {
  const layoutTolerance = 0.25;
  return (
    left.index === right.index &&
    Math.abs(left.defaultY - right.defaultY) < layoutTolerance &&
    Math.abs(left.minY - right.minY) < layoutTolerance &&
    Math.abs(left.maxY - right.maxY) < layoutTolerance &&
    Math.abs(left.labelY - right.labelY) < layoutTolerance
  );
}

/** Read one finite node Y without trusting host geometry. */
function readNodeY(node: ComfyNode): number {
  return finite(node.pos?.[1], 0);
}

/** Resolve one renderer-owned origin without trusting an invalid adapter result. */
function resolvePresentationOrigin(node: ComfyNode, presentation: NodePresentation): number {
  return finite(presentation.resolveOriginY(), readNodeY(node));
}

/** Return one finite number or its explicit fallback. */
function finite(value: unknown, fallback: number): number {
  const number = Number(value);
  return Number.isFinite(number) ? number : fallback;
}

/** Clamp one number to an inclusive finite range. */
function clamp(value: number, minimum: number, maximum: number): number {
  return Math.min(Math.max(value, minimum), maximum);
}
