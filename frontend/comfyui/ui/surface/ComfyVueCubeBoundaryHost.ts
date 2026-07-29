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
/** Position graph-owned Nodes 2.0 slots on animated Cube boundary gutters. */

import type { CubePortPresentationController } from '../cube/connection/CubePortPresentationController.js';
import {
  resolveCubeExternalInterface,
  type CubeExternalInterfaceNode,
} from '../cube/graph/CubeExternalInterface.js';
import type { ComfyNode } from '../types/graph.js';
import {
  measureCubeBoundary,
  measureSocketRadius,
  readLocalOffset,
  readVueNodeGraphY,
  type CubeBoundaryMeasurement,
} from './CubeDomBoundaryGeometry.js';
import { CubeDomPortLeaderHost } from './CubeDomPortLeaderHost.js';

interface ElementPresentation {
  style: string | null;
  hidden: boolean;
  boundaryRow: string | undefined;
  boundaryDirection: string | undefined;
  boundaryIndex: string | undefined;
}

interface BoundarySlotBinding {
  element: HTMLElement;
  index: number;
}

export interface ComfyVueCubeBoundaryHostOptions {
  body: HTMLElement;
  node: ComfyNode;
  titleHeight?: number;
  portPresentation?: CubePortPresentationController;
  requestSlotLayoutSync?: () => void;
}

/** Own only structural positioning for Comfy's actual native slot elements. */
export class ComfyVueCubeBoundaryHost {
  readonly #body: HTMLElement;
  readonly #node: ComfyNode;
  readonly #titleHeight: number;
  readonly #portPresentation: CubePortPresentationController | null;
  readonly #requestSlotLayoutSync: () => void;
  readonly #resolveOriginY: () => number;
  readonly #presentations = new Map<HTMLElement, ElementPresentation>();
  readonly #inputLabels = new Map<HTMLElement, string | null>();
  readonly #leaders: CubeDomPortLeaderHost;
  readonly #unsubscribe: () => void;
  readonly #measurements: Record<'input' | 'output', CubeBoundaryMeasurement | null> = {
    input: null,
    output: null,
  };
  #lastSyncedLayout = '';

  /** Bind one current native node body. */
  constructor(options: ComfyVueCubeBoundaryHostOptions) {
    this.#body = options.body;
    this.#node = options.node;
    this.#titleHeight = Math.max(0, Number(options.titleHeight) || 30);
    this.#portPresentation = options.portPresentation ?? null;
    this.#requestSlotLayoutSync = options.requestSlotLayoutSync ?? (() => undefined);
    const nodeRoot = this.#body.closest<HTMLElement>('.lg-node') ?? this.#body;
    this.#resolveOriginY = () =>
      readVueNodeGraphY(nodeRoot, this.#titleHeight, Number(this.#node.pos?.[1]) || 0);
    this.#portPresentation?.registerOrigin(this.#node, this.#resolveOriginY);
    this.#leaders = new CubeDomPortLeaderHost(options.body);
    this.#unsubscribe =
      this.#portPresentation?.subscribe(options.node, () => this.#renderAnimationFrame()) ??
      (() => undefined);
    this.reconcile();
  }

  /** Reapply markers after Comfy replaces slot descendants. */
  reconcile(): void {
    this.#portPresentation?.registerOrigin(this.#node, this.#resolveOriginY);
    const row = [...this.#body.children].find(
      (child): child is HTMLElement =>
        child instanceof HTMLElement && child.querySelector('.lg-slot') !== null,
    );
    if (!row) return;
    const inputSlots = [...row.querySelectorAll<HTMLElement>('.lg-slot--input')];
    const outputSlots = [...row.querySelectorAll<HTMLElement>('.lg-slot--output')];
    const externalInterface = resolveNativeCubeExternalInterface(this.#node);
    const inputs = this.#applySlotVisibility(
      inputSlots,
      externalInterface?.inputSlots ?? inputSlots.map((_, index) => index),
    );
    const outputs = this.#applySlotVisibility(
      outputSlots,
      externalInterface?.outputSlots ?? outputSlots.map((_, index) => index),
    );
    const outputTitles = measureOutputTitles(this.#body, row, outputs.length);
    const nodeRoot = this.#body.closest<HTMLElement>('.lg-node') ?? this.#body;
    const priorInputMeasurement = this.#measurements.input;
    const stableInputRowPositions =
      priorInputMeasurement?.anchors.length === inputs.length
        ? priorInputMeasurement.anchors.map(
            (anchor) =>
              anchor.defaultY +
              priorInputMeasurement.graphOriginOffset -
              priorInputMeasurement.rowOffset,
          )
        : [];
    this.#measurements.input = measureCubeBoundary(
      this.#body,
      nodeRoot,
      row,
      inputs.map(({ element }) => element),
      stableInputRowPositions,
      this.#titleHeight,
    );
    this.#measurements.output = measureCubeBoundary(
      this.#body,
      nodeRoot,
      row,
      outputs.map(({ element }) => element),
      outputTitles.map((item) => item.y),
      this.#titleHeight,
    );
    this.#remember(row);
    row.dataset.sugarcubeBoundaryRow = '';
    this.#markSlots('input', inputs, true);
    this.#markSlots('output', outputs, true);
    this.#renderLeaders(row, outputs, outputTitles);
    this.#requestLayoutSyncForNewGeometry(inputs, outputs);
  }

  /** Paint transient positions without feeding animated geometry back into anchor measurement. */
  #renderAnimationFrame(): void {
    const row = [...this.#body.children].find(
      (child): child is HTMLElement =>
        child instanceof HTMLElement && child.querySelector('.lg-slot') !== null,
    );
    if (!row) return;
    const inputSlots = [...row.querySelectorAll<HTMLElement>('.lg-slot--input')];
    const outputSlots = [...row.querySelectorAll<HTMLElement>('.lg-slot--output')];
    const externalInterface = resolveNativeCubeExternalInterface(this.#node);
    const inputs = this.#applySlotVisibility(
      inputSlots,
      externalInterface?.inputSlots ?? inputSlots.map((_, index) => index),
    );
    const outputs = this.#applySlotVisibility(
      outputSlots,
      externalInterface?.outputSlots ?? outputSlots.map((_, index) => index),
    );
    const outputTitles = measureOutputTitles(this.#body, row, outputs.length);
    this.#markSlots('input', inputs, false);
    this.#markSlots('output', outputs, false);
    this.#renderLeaders(row, outputs, outputTitles);
    this.#requestLayoutSyncForNewGeometry(inputs, outputs);
  }

  /** Ask Comfy to remeasure once per semantic boundary geometry revision. */
  #requestLayoutSyncForNewGeometry(
    inputSlots: readonly BoundarySlotBinding[],
    outputSlots: readonly BoundarySlotBinding[],
  ): void {
    if (this.#portPresentation?.isAnimating(this.#node)) return;
    const signature = [
      serializeSlotPositions(
        'input',
        inputSlots.map(({ element }) => element),
      ),
      serializeSlotPositions(
        'output',
        outputSlots.map(({ element }) => element),
      ),
    ].join('|');
    if (signature === this.#lastSyncedLayout) return;
    this.#lastSyncedLayout = signature;
    this.#requestSlotLayoutSync();
  }

  /** Update stable leader elements from current title and socket positions. */
  #renderLeaders(
    row: HTMLElement,
    outputSlots: readonly BoundarySlotBinding[],
    outputTitles: readonly MeasuredOutputTitle[],
  ): void {
    const rowOffset = readLocalOffset(this.#body, row);
    this.#leaders.render(
      outputTitles.flatMap((item, position) => {
        const binding = outputSlots[position];
        const portY = readBoundaryPosition(binding?.element);
        return portY === null
          ? []
          : [
              {
                index: binding?.index ?? position,
                title: item.title,
                portY: rowOffset + portY,
                socketRadius: measureSocketRadius(this.#body, binding?.element),
              },
            ];
      }),
    );
  }

  /** Restore every Comfy-owned element exactly as it was mounted. */
  dispose(): void {
    this.#unsubscribe();
    this.#leaders.dispose();
    for (const [label, text] of this.#inputLabels) label.textContent = text;
    this.#inputLabels.clear();
    for (const [element, presentation] of this.#presentations) {
      restoreAttribute(element, 'style', presentation.style);
      element.hidden = presentation.hidden;
      restoreDataset(element, 'sugarcubeBoundaryRow', presentation.boundaryRow);
      restoreDataset(element, 'sugarcubeBoundaryDirection', presentation.boundaryDirection);
      restoreDataset(element, 'sugarcubeBoundaryIndex', presentation.boundaryIndex);
    }
    this.#presentations.clear();
  }

  /** Mark one direction's native slots from its stable measured geometry. */
  #markSlots(
    direction: 'input' | 'output',
    slots: readonly BoundarySlotBinding[],
    registerAnchors: boolean,
  ): boolean {
    const measurement = this.#measurements[direction];
    if (!measurement) return false;
    if (registerAnchors) {
      this.#portPresentation?.register(
        this.#node,
        direction,
        measurement.anchors.map((anchor, position) => ({
          ...anchor,
          index: slots[position]?.index ?? position,
        })),
      );
    }
    let changed = false;
    for (const [position, { element: slot, index }] of slots.entries()) {
      this.#remember(slot);
      const boundaryIndex = String(index);
      const boundaryPosition =
        (this.#portPresentation?.resolveLocalY(this.#node, direction, index) ??
          measurement.anchors[position]?.defaultY ??
          measurement.rowMinimumY + measurement.rowOffset - measurement.graphOriginOffset) +
        measurement.graphOriginOffset -
        measurement.rowOffset;
      const boundaryPositionCss = `${String(
        clamp(boundaryPosition, measurement.rowMinimumY, measurement.rowMaximumY),
      )}px`;
      changed =
        slot.dataset.sugarcubeBoundaryDirection !== direction ||
        slot.dataset.sugarcubeBoundaryIndex !== boundaryIndex ||
        slot.style.getPropertyValue('--sugarcube-boundary-position') !== boundaryPositionCss ||
        changed;
      slot.dataset.sugarcubeBoundaryDirection = direction;
      slot.dataset.sugarcubeBoundaryIndex = boundaryIndex;
      slot.style.setProperty('--sugarcube-boundary-position', boundaryPositionCss);
      if (direction === 'input') this.#presentInputLabel(slot, index);
    }
    return changed;
  }

  /** Keep dormant authoring boundaries in the editor but out of the closed face. */
  #applySlotVisibility(
    slots: readonly HTMLElement[],
    visibleIndexes: readonly number[],
  ): BoundarySlotBinding[] {
    const visible = new Set(visibleIndexes);
    return slots.flatMap((element, index) => {
      this.#remember(element);
      const isVisible = visible.has(index);
      element.hidden = !isVisible;
      if (isVisible) {
        element.style.removeProperty('display');
        return [{ element, index }];
      }
      // Comfy's utility classes set display:flex, which overrides the HTML
      // hidden attribute in Nodes 2.0. Use an explicit priority so dormant
      // draft ports cannot leak onto the closed Cube face.
      element.style.setProperty('display', 'none', 'important');
      return [];
    });
  }

  /** Render a concise type label without mutating native slot metadata. */
  #presentInputLabel(slot: HTMLElement, index: number): void {
    const label = slot.querySelector<HTMLElement>('.text-node-component-slot-text');
    if (!label) return;
    if (!this.#inputLabels.has(label)) this.#inputLabels.set(label, label.textContent);
    const type = readPortType(this.#node.inputs?.[index]?.type);
    if (type && label.textContent !== type) label.textContent = type;
  }

  /** Retain host presentation once before applying Cube markers. */
  #remember(element: HTMLElement): void {
    if (this.#presentations.has(element)) return;
    this.#presentations.set(element, {
      style: element.getAttribute('style'),
      hidden: element.hidden,
      boundaryRow: element.dataset.sugarcubeBoundaryRow,
      boundaryDirection: element.dataset.sugarcubeBoundaryDirection,
      boundaryIndex: element.dataset.sugarcubeBoundaryIndex,
    });
  }
}

/** Degrade safely when a host replaces a Cube node during native reconciliation. */
function resolveNativeCubeExternalInterface(node: ComfyNode) {
  const subgraph = node.subgraph;
  if (!isCubeExternalInterfaceNode(subgraph)) return null;
  return resolveCubeExternalInterface({
    ...(node.inputs ? { inputs: node.inputs } : {}),
    ...(node.outputs ? { outputs: node.outputs } : {}),
    ...(node.properties ? { properties: node.properties } : {}),
    subgraph,
  });
}

/** Validate only the native object boundary consumed by the interface policy. */
function isCubeExternalInterfaceNode(
  value: unknown,
): value is CubeExternalInterfaceNode['subgraph'] {
  return typeof value === 'object' && value !== null && !Array.isArray(value);
}

/** Restore one nullable host attribute. */
function restoreAttribute(element: HTMLElement, name: string, value: string | null): void {
  if (value === null) {
    element.removeAttribute(name);
  } else {
    element.setAttribute(name, value);
  }
}

/** Restore one optional host dataset value. */
function restoreDataset(
  element: HTMLElement,
  key: 'sugarcubeBoundaryRow' | 'sugarcubeBoundaryDirection' | 'sugarcubeBoundaryIndex',
  value: string | undefined,
): void {
  if (value === undefined) {
    element.removeAttribute(DATASET_ATTRIBUTES[key]);
  } else {
    element.dataset[key] = value;
  }
}

const DATASET_ATTRIBUTES: Record<
  'sugarcubeBoundaryRow' | 'sugarcubeBoundaryDirection' | 'sugarcubeBoundaryIndex',
  string
> = {
  sugarcubeBoundaryRow: 'data-sugarcube-boundary-row',
  sugarcubeBoundaryDirection: 'data-sugarcube-boundary-direction',
  sugarcubeBoundaryIndex: 'data-sugarcube-boundary-index',
};

interface MeasuredOutputTitle {
  title: HTMLElement;
  y: number;
}

/** Measure preview-title centers in the row's unscaled CSS coordinate space. */
function measureOutputTitles(
  body: HTMLElement,
  row: HTMLElement,
  count: number,
): MeasuredOutputTitle[] {
  const titles = [...body.querySelectorAll<HTMLElement>('[data-cube-preview-output-title]')].slice(
    0,
    count,
  );
  if (titles.length !== count || count === 0) return [];
  const bodyRect = body.getBoundingClientRect();
  const rowRect = row.getBoundingClientRect();
  const scale = body.offsetWidth > 0 ? bodyRect.width / body.offsetWidth : 1;
  const safeScale = Number.isFinite(scale) && scale > 0 ? scale : 1;
  return titles.map((title) => {
    const titleRect = title.getBoundingClientRect();
    const position = (titleRect.top + titleRect.height / 2 - rowRect.top) / safeScale;
    return { title, y: Math.max(0, position) };
  });
}

/** Read one currently applied pixel position for output leader geometry. */
function readBoundaryPosition(slot: HTMLElement | undefined): number | null {
  const value = slot?.style.getPropertyValue('--sugarcube-boundary-position');
  if (!value) return null;
  const parsed = Number.parseFloat(value);
  return Number.isFinite(parsed) ? parsed : null;
}

/** Serialize stable presentation geometry independently from Vue element identity. */
function serializeSlotPositions(
  direction: 'input' | 'output',
  slots: readonly HTMLElement[],
): string {
  return `${direction}:${slots
    .map((slot) => slot.style.getPropertyValue('--sugarcube-boundary-position'))
    .join(',')}`;
}

/** Read one concise native type while treating host text as untrusted content. */
function readPortType(value: unknown): string {
  if (typeof value === 'string') return value.trim();
  if (!Array.isArray(value)) return '';
  return value
    .filter((item): item is string => typeof item === 'string')
    .map((item) => item.trim())
    .filter(Boolean)
    .join(' | ');
}

/** Clamp one rendered row coordinate to its measured Cube-body travel range. */
function clamp(value: number, minimum: number, maximum: number): number {
  return Math.min(Math.max(value, minimum), maximum);
}
