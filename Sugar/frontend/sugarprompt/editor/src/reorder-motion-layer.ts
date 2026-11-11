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
/** One native ViewPlugin owns a zero-flow scrollDOM sibling and its viewport-clipped finite Stage lease. */
import { requireValue } from '../../core/src/required.js';
import { ViewPlugin, type EditorView, type ViewUpdate } from '@codemirror/view';
import { admitsReorderMotionPaintContext } from './reorder-motion-paint-admission.js';
import {
  captureDeclarations,
  matchesDeclarations,
  type DeclarationWitness,
} from './reorder-motion-declarations.js';
/** Registrations exist only between native ViewPlugin construction and destruction. */
const owners = new WeakMap<EditorView, ReorderMotionAttachment>();
/** An admitted clip is borrowed only after native draw and a subsequent extent-checking READ. */
export interface ReorderMotionLayerLease {
  current(): boolean;
  read(): 'waiting' | 'ready' | 'unavailable';
  release(): void;
}
/** A READ-issued native scrollport rectangle retains the original unpolluted scroller dimensions. */
interface ClipPlan {
  readonly issue: object;
  readonly left: number;
  readonly top: number;
  readonly width: number;
  readonly height: number;
  readonly extent: readonly number[];
  readonly viewport: readonly number[];
}
/** The Stage keeps its existing calibration; its initially clipped box supplies actual marker-axis scale. */
interface BorrowedStage {
  readonly node: HTMLElement;
  readonly width: number;
  readonly retire: () => void;
  readonly issue: object;
}
/** The finite internal clip box cannot be displaced or expanded by author-origin host declarations. */
const fixedClipDeclarations = [
  ['position', 'absolute'],
  ['overflow', 'clip'],
  ['overflow-clip-margin', '0px'],
  ['margin', '0'],
  ['padding', '0'],
  ['border', '0'],
  ['box-sizing', 'border-box'],
] as const;
/** The current measured lease borrows only the actual native scroller's two-axis clipping owner. */
function admitsNativeScrollerClip(view: EditorView, current: () => boolean): boolean {
  const window = view.dom.ownerDocument.defaultView;
  if (!window || !current()) return false;
  const style = window.getComputedStyle(view.scrollDOM);
  if (!current()) return false;
  const x = style.getPropertyValue('overflow-x');
  if (!current()) return false;
  const y = style.getPropertyValue('overflow-y');
  return current() && x === 'auto' && y === 'auto';
}
/** Native READ certifies this live mount delegates to the separately admitted genuine scrollport clip. */
function admitsMountClip(node: HTMLElement, window: Window): boolean {
  const style = window.getComputedStyle(node);
  return (
    style.getPropertyValue('overflow-x') === 'visible' &&
    style.getPropertyValue('overflow-y') === 'visible' &&
    style.getPropertyValue('overflow-clip-margin') === '0px' &&
    ['top', 'right', 'bottom', 'left'].every((side) =>
      ['border-' + side + '-width', 'padding-' + side].every(
        (name) => style.getPropertyValue(name) === '0px',
      ),
    ) &&
    fixedClipDeclarations.every(([name]) => node.style.getPropertyPriority(name) === 'important') &&
    ['left', 'top', 'width', 'height', 'transform'].every(
      (name) => node.style.getPropertyPriority(name) === 'important',
    )
  );
}
/** One ViewPlugin lifetime owns only outside-content attachment, clipping and the borrowed Stage. */
export class ReorderMotionAttachment {
  private node: HTMLElement | null = null;
  private borrowed: BorrowedStage | null = null;
  private drawn: ClipPlan | null = null;
  private prepared: ClipPlan | null = null;
  private disposed = false;
  private declarations: DeclarationWitness | null = null;
  private writes = 0;
  private readonly view: EditorView;
  private readonly root: HTMLElement;
  private rootDeclarations: DeclarationWitness | null = null;
  /** One keyed request shares CodeMirror's native READ/WRITE queue; retirement makes captured callbacks inert. */
  private readonly measurement = {
    key: this,
    read: (): ClipPlan | null => this.readClip(),
    write: (plan: ClipPlan | null): void => {
      const node = this.node;
      if (!this.disposed && owners.get(this.view) === this && node) this.write(node, plan);
    },
  };
  /** The public scroller owns this absolute zero-size sibling; no CM layer applies inverse scale. */
  constructor(view: EditorView) {
    this.view = view;
    if (owners.has(view)) throw new Error('reorder-attachment-duplicate-mount');
    const root = view.dom.ownerDocument.createElement('div');
    this.root = root;
    try {
      if (owners.has(view)) throw new Error('reorder-attachment-duplicate-mount');
      owners.set(view, this);
      root.className = 'sp-reorder-motion-layer';
      root.inert = true;
      root.setAttribute('aria-hidden', 'true');
      const style = root.style;
      for (const [name, value] of [
        ['position', 'absolute'],
        ['left', '0px'],
        ['top', '0px'],
        ['width', '0px'],
        ['height', '0px'],
        ['margin', '0'],
        ['padding', '0'],
        ['border', '0'],
        ['overflow', 'visible'],
        ['contain', 'layout size'],
        ['transform', 'none'],
        ['transform-origin', '0 0'],
        ['pointer-events', 'none'],
        ['z-index', '-1'],
      ] as const) {
        if (this.disposed || owners.get(view) !== this)
          throw new Error('reorder-attachment-retired-during-mount');
        style.setProperty(name, value, 'important');
      }
      const rootWitness = captureDeclarations(
        root,
        style,
        () => !this.disposed && owners.get(view) === this,
      );
      if (!rootWitness) throw new Error('reorder-attachment-mount-unavailable');
      this.rootDeclarations = rootWitness;
      view.scrollDOM.append(root);
      if (this.disposed || owners.get(view) !== this || root.parentElement !== view.scrollDOM)
        throw new Error('reorder-attachment-mount-unavailable');
      const node = this.draw(null);
      root.append(node);
      if (!this.mounted(false)) throw new Error('reorder-attachment-mount-unavailable');
    } catch (primary) {
      const causes: unknown[] = [primary];
      // A delegated append may finish after reentrant destruction; retire the captured physical root independently.
      for (const release of [() => this.destroy(), () => root.remove()])
        try {
          release();
        } catch (cause) {
          causes.push(cause);
        }
      if (causes.length === 1) throw primary;
      throw new AggregateError(causes, 'reorder-attachment-construction-failed');
    }
  }
  /** Native geometry/viewport updates schedule only this attachment's current measured clipping work. */
  update(update: Pick<ViewUpdate, 'geometryChanged' | 'viewportChanged'>): void {
    if (update.geometryChanged || update.viewportChanged) this.schedule();
  }
  /** Attachment requests measurement directly; no source-neutral transaction or layer refresh effect is needed. */
  private schedule(): void {
    if (!this.disposed && owners.get(this.view) === this)
      this.view.requestMeasure(this.measurement);
  }
  /** Delegated declaration reads cannot borrow another generation or an ancestry that retired during the getter. */
  current(): boolean {
    return this.mounted(true);
  }
  /** The same structural/style authority admits detached construction; only ready leases require actual document connection. */
  private mounted(requireConnection: boolean): boolean {
    const node = this.node,
      declarations = this.declarations,
      borrowed = this.borrowed,
      drawn = this.drawn;
    const owned = (): boolean =>
      !this.disposed &&
      this.writes === 0 &&
      owners.get(this.view) === this &&
      this.node === node &&
      this.declarations === declarations &&
      this.borrowed === borrowed &&
      this.drawn === drawn;
    if (!node || declarations === null || !owned()) return false;
    if (!matchesDeclarations(node, declarations.text, declarations.spelling, owned)) return false;
    if (
      this.rootDeclarations === null ||
      !matchesDeclarations(
        this.root,
        this.rootDeclarations.text,
        this.rootDeclarations.spelling,
        owned,
      )
    )
      return false;
    const attached =
      (!requireConnection || node.isConnected) &&
      node.parentElement === this.root &&
      this.root.parentElement === this.view.scrollDOM &&
      this.view.scrollDOM.parentElement === this.view.dom;
    return attached && owned();
  }
  /** Native READ uses the original scrollport and inherited-scale hidden Stage calibration box. */
  private readClip(): ClipPlan | null {
    const stage = this.borrowed;
    if (!stage || !this.current()) return null;
    /** A delegated style read cannot transfer this READ to another borrowed Stage issue. */
    const current = (): boolean =>
      this.borrowed === stage && this.current() && this.borrowed === stage;
    if (!admitsNativeScrollerClip(this.view, current)) return null;
    if (this.prepared?.issue === stage.issue) return this.prepared;
    const scroll = this.view.scrollDOM,
      box = scroll.getBoundingClientRect(),
      origin = this.root.getBoundingClientRect(),
      calibration = stage.node.getBoundingClientRect(),
      scaleX = calibration.width / stage.width,
      scaleY = calibration.height / 256;
    if (!admitsReorderMotionPaintContext(this.view, this.root)) return null;
    const width = (scroll.clientWidth * this.view.scaleX) / scaleX,
      height = (scroll.clientHeight * this.view.scaleY) / scaleY,
      left = (box.left + scroll.clientLeft * this.view.scaleX - origin.left) / scaleX,
      top = (box.top + scroll.clientTop * this.view.scaleY - origin.top) / scaleY;
    const extent = [
      scroll.scrollWidth,
      scroll.scrollHeight,
      scroll.clientWidth,
      scroll.clientHeight,
      scroll.scrollLeft,
      scroll.scrollTop,
    ];
    if (
      ![left, top, width, height, scaleX, scaleY, ...extent].every(Number.isFinite) ||
      width <= 0 ||
      height <= 0 ||
      scaleX <= 0 ||
      scaleY <= 0 ||
      extent.some((value) => value < 0)
    )
      return null;
    const viewport = [
      box.left + scroll.clientLeft * this.view.scaleX,
      box.top + scroll.clientTop * this.view.scaleY,
      scroll.clientWidth * this.view.scaleX,
      scroll.clientHeight * this.view.scaleY,
    ];
    this.prepared = Object.freeze({
      issue: stage.issue,
      left,
      top,
      width,
      height,
      extent: Object.freeze(extent),
      viewport: Object.freeze(viewport),
    });
    return this.prepared;
  }
  /** Construction creates one empty clipping boundary before any Stage can attach. */
  private draw(plan: ClipPlan | null): HTMLElement {
    if (this.node || this.disposed) throw new Error('reorder-layer-marker-redraw');
    const node = this.view.dom.ownerDocument.createElement('span');
    node.className = 'sp-reorder-motion-mount';
    node.inert = true;
    node.setAttribute('aria-hidden', 'true');
    Object.assign(node.style, { pointerEvents: 'none' });
    this.node = node;
    this.write(node, plan);
    return node;
  }
  /** Stamp only the captured declaration after its getter and exact attachment ancestry remain owned. */
  private stamp(node: HTMLElement, style: CSSStyleDeclaration, owned: () => boolean): boolean {
    const parent = node.parentElement;
    if (!owned()) return false;
    const witness = captureDeclarations(node, style, owned);
    if (!witness) return false;
    const attached =
      node.parentElement === parent &&
      this.root.parentElement === this.view.scrollDOM &&
      this.view.scrollDOM.parentElement === this.view.dom;
    if (!attached || !owned()) return false;
    this.declarations = witness;
    return true;
  }
  /** Every captured physical reset is attempted even after an earlier setter throws or the owner retires. */
  private clearCaptured(node: HTMLElement, style: CSSStyleDeclaration): unknown[] {
    if (this.node === node) this.declarations = this.drawn = null;
    const causes: unknown[] = [];
    for (const [name, value] of [
      ...fixedClipDeclarations,
      ['left', '0px'],
      ['top', '0px'],
      ['transform', 'none'],
      ['width', '0px'],
      ['height', '0px'],
    ] as const) {
      try {
        style.setProperty(name, value, 'important');
      } catch (cause) {
        causes.push(cause);
      }
    }
    if (!causes.length && this.borrowed === null) {
      const owned = (): boolean =>
        !this.disposed &&
        owners.get(this.view) === this &&
        this.node === node &&
        this.borrowed === null &&
        this.drawn === null &&
        this.declarations === null;
      try {
        if (owned()) this.stamp(node, style, owned);
      } catch (cause) {
        causes.push(cause);
      }
    }
    return causes;
  }
  /** Reserve actual native writes through captured late cleanup; Stage attachment cannot dispatch inside this attachment WRITE. */
  write(node: HTMLElement, plan: ClipPlan | null): void {
    if (this.disposed || this.node !== node) throw new Error('reorder-layer-marker-retired');
    const borrowed = this.borrowed,
      parent = node.parentElement;
    if (plan && plan.issue !== borrowed?.issue) plan = null;
    this.drawn = plan;
    this.declarations = null;
    ++this.writes;
    let style: CSSStyleDeclaration | null = null,
      completed = false;
    const causes: unknown[] = [];
    const owned = (): boolean =>
      !this.disposed &&
      owners.get(this.view) === this &&
      this.node === node &&
      this.borrowed === borrowed &&
      this.drawn === plan &&
      this.declarations === null &&
      node.parentElement === parent &&
      this.root.parentElement === this.view.scrollDOM &&
      this.view.scrollDOM.parentElement === this.view.dom;
    try {
      style = node.style;
      let active = owned();
      for (const [name, value] of [
        ...fixedClipDeclarations,
        ['left', '0px'],
        ['top', '0px'],
        ['transform', plan ? `translate(${plan.left}px,${plan.top}px)` : 'none'],
        ['width', `${plan?.width ?? 0}px`],
        ['height', `${plan?.height ?? 0}px`],
      ] as const) {
        if (!active) break;
        style.setProperty(name, name === 'overflow' && plan ? 'visible' : value, 'important');
        active = owned();
      }
      if (active) completed = this.stamp(node, style, owned);
    } catch (cause) {
      causes.push(cause);
    } finally {
      try {
        if (!completed && style) causes.push(...this.clearCaptured(node, style));
      } finally {
        --this.writes;
      }
    }
    if (causes.length === 1) throw causes[0];
    if (causes.length) throw new AggregateError(causes, 'reorder-layer-write-retirement-failed');
  }
  /** One Stage can occupy the exact connected clip under its native scroller sibling. */
  attach(node: HTMLElement, width: number, retire: () => void): ReorderMotionLayerLease | null {
    if (!this.current() || this.borrowed || !this.node) return null;
    const borrowed: BorrowedStage = { node, width, retire, issue: {} };
    this.borrowed = borrowed;
    this.prepared = null;
    try {
      this.node.append(node);
      this.schedule();
    } catch (cause) {
      try {
        this.empty(borrowed);
      } catch (cleanup) {
        throw new AggregateError([cause, cleanup], 'reorder-layer-attachment-failed');
      }
      throw cause;
    }
    const current = (): boolean =>
      this.current() && this.borrowed === borrowed && node.parentElement === this.node;
    return {
      current,
      /** A subsequent native READ certifies exact original scroll extents before coverage/reveal. */
      read: () => {
        if (!current()) return 'unavailable';
        if (!admitsNativeScrollerClip(this.view, current)) return 'unavailable';
        const plan = this.drawn;
        if (!plan || plan.issue !== borrowed.issue) return 'waiting';
        const scroll = this.view.scrollDOM,
          extent = [
            scroll.scrollWidth,
            scroll.scrollHeight,
            scroll.clientWidth,
            scroll.clientHeight,
            scroll.scrollLeft,
            scroll.scrollTop,
          ];
        if (extent.some((value, index) => value !== plan.extent[index])) return 'unavailable';
        const mount = requireValue(this.node, 'reorder-motion-layer'),
          window = this.view.dom.ownerDocument.defaultView;
        if (!window || !admitsMountClip(mount, window) || !current()) return 'unavailable';
        const box = mount.getBoundingClientRect();
        if (
          [box.left, box.top, box.width, box.height].some(
            (value, index) =>
              Math.abs(value - requireValue(plan.viewport[index], 'reorder-motion-layer')) > 1 / 64,
          )
        )
          return 'unavailable';
        return current() ? 'ready' : 'unavailable';
      },
      /** Retirement removes the old viewport box before another source's extents can be measured. */
      release: () => this.empty(borrowed),
    };
  }
  /** An exact old issue cannot empty a newer publication; no geometry is read while retiring. */
  private empty(stage: BorrowedStage): void {
    if (this.borrowed !== stage) return;
    this.borrowed = null;
    this.prepared = this.drawn = null;
    if (this.node) this.write(this.node, null);
  }
  /** ViewPlugin destruction attempts exact root removal even when Stage cleanup throws. */
  destroy(): void {
    if (this.disposed) return;
    this.disposed = true;
    const borrowed = this.borrowed;
    this.borrowed = null;
    this.node = null;
    this.declarations = null;
    this.rootDeclarations = null;
    this.prepared = this.drawn = null;
    if (owners.get(this.view) === this) owners.delete(this.view);
    const failures: unknown[] = [];
    for (const release of [() => borrowed?.retire(), () => this.root.remove()]) {
      try {
        release();
      } catch (cause) {
        failures.push(cause);
      }
    }
    if (failures.length === 1) throw failures[0];
    if (failures.length) throw new AggregateError(failures, 'reorder-layer-retirement-failed');
  }
}
/** Preparation waits for the genuine empty native attachment; that box cannot export Stage overflow. */
export function reorderMotionAttachmentReady(view: EditorView): boolean {
  return owners.get(view)?.current() ?? false;
}
/** Stage construction borrows only the installed native attachment. */
export function attachReorderMotionStage(
  view: EditorView,
  node: HTMLElement,
  width: number,
  retire: () => void,
): ReorderMotionLayerLease | null {
  return owners.get(view)?.attach(node, width, retire) ?? null;
}
/** The finite ViewPlugin creates only an owned scrollDOM sibling; native source/caret/history remain untouched. */
export const reorderMotionAttachmentExtension = ViewPlugin.fromClass(ReorderMotionAttachment);
