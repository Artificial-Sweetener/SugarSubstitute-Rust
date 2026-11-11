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
/** One admitted materialization owns at most 64 detached nonpixel style prototypes. */
import type { PreparedReorderPaintNode } from './reorder-paint-preparation.js';
/** Own bounded inert style prototypes for one admitted paint materialization. */
export class ReorderPaintMaterialization {
  private document: Document | null;
  private readonly prototypes = new Map<string, HTMLElement>();
  private readonly attempted = new Set<HTMLElement>();
  private allocating = 0;
  /** Construction allocates no native destination before deferred currentness admission. */
  constructor(document: Document) {
    this.document = document;
  }
  /** Exact namespace/tag/declarations reuse only parsed inert style; literal data and pixels remain per copy. */
  create(descriptor: Extract<PreparedReorderPaintNode, { readonly kind: 'element' }>): HTMLElement {
    const document = this.document;
    if (!document) throw new Error('reorder-paint-materialization-disposed');
    const namespace = descriptor.vector
      ? 'http://www.w3.org/2000/svg'
      : 'http://www.w3.org/1999/xhtml';
    const key = JSON.stringify([namespace, descriptor.tag, descriptor.cssText]);
    const reusable = !descriptor.media && !descriptor.buffer && descriptor.tag !== 'canvas';
    const prototype = reusable ? this.prototypes.get(key) : undefined;
    if (prototype) return this.admit(document, prototype.cloneNode(false) as HTMLElement);
    const prototypeOwned = reusable && this.attempted.size + this.allocating < 64;
    if (prototypeOwned) ++this.allocating;
    let node: HTMLElement;
    try {
      node = descriptor.vector
        ? (document.createElementNS(namespace, descriptor.tag) as unknown as HTMLElement)
        : document.createElement(descriptor.tag);
    } finally {
      if (prototypeOwned) --this.allocating;
    }
    if (prototypeOwned) this.attempted.add(node);
    try {
      this.assertCurrent(document);
      node.style.cssText = descriptor.cssText;
      this.assertCurrent(document);
      node.style.setProperty('animation', 'none', 'important');
      this.assertCurrent(document);
      node.style.setProperty('transition', 'none', 'important');
      this.assertCurrent(document);
      node.style.setProperty('pointer-events', 'none', 'important');
      this.assertCurrent(document);
      node.style.setProperty('-webkit-user-modify', 'read-only', 'important');
      this.assertCurrent(document);
      node.style.setProperty('user-select', 'none', 'important');
      this.assertCurrent(document);
      if (prototypeOwned) {
        this.prototypes.set(key, node);
        return this.admit(document, node.cloneNode(false) as HTMLElement);
      }
      return node;
    } catch (primary) {
      if (this.document !== document && (!prototypeOwned || this.attempted.has(node))) {
        this.attempted.delete(node);
        try {
          node.remove();
        } catch (cleanup) {
          throw new AggregateError([primary, cleanup], 'reorder-paint-retired-node-removal-failed');
        }
      }
      throw primary;
    }
  }
  /** External native operations cannot republish maps or return a clone after request retirement. */
  private assertCurrent(document: Document): void {
    if (this.document !== document) throw new Error('reorder-paint-materialization-retired');
  }
  /** A clone created during retirement is still owned until admission and must be independently removed. */
  private admit(document: Document, node: HTMLElement): HTMLElement {
    try {
      this.assertCurrent(document);
    } catch (primary) {
      try {
        node.remove();
      } catch (cleanup) {
        throw new AggregateError([primary, cleanup], 'reorder-paint-retired-clone-removal-failed');
      }
      throw primary;
    }
    return node;
  }
  /** Retire every attempted detached template independently; release references before external removals can reenter. */
  retire(): readonly unknown[] {
    this.document = null;
    this.prototypes.clear();
    const nodes = [...this.attempted];
    this.attempted.clear();
    const causes: unknown[] = [];
    for (const node of nodes) {
      try {
        node.remove();
      } catch (cause) {
        causes.push(cause);
      }
    }
    return causes;
  }
  /** Final request disposal preserves all native removal failures and is idempotent after any outcome. */
  dispose(): void {
    const causes = this.retire();
    if (causes.length === 1) throw causes[0];
    if (causes.length)
      throw new AggregateError(causes, 'reorder-paint-prototype-retirement-failed');
  }
}
/** One complete request owns successful output until template retirement succeeds, with explicit rollback on failed transfer. */
export function withReorderPaintMaterialization<Result>(
  document: Document,
  action: (materialization: ReorderPaintMaterialization) => Result,
  rollback: (output: Result) => void,
): Result {
  const materialization = new ReorderPaintMaterialization(document);
  let outcome:
    | { readonly kind: 'success'; readonly output: Result }
    | { readonly kind: 'failure'; readonly cause: unknown };
  try {
    outcome = { kind: 'success', output: action(materialization) };
  } catch (cause) {
    outcome = { kind: 'failure', cause };
  }
  const causes = materialization.retire();
  if (outcome.kind === 'failure') {
    if (causes.length)
      throw new AggregateError(
        [outcome.cause, ...causes],
        'reorder-paint-materialization-retirement-failed',
      );
    throw outcome.cause;
  }
  if (causes.length) {
    const primary =
      causes.length === 1
        ? causes[0]
        : new AggregateError(causes, 'reorder-paint-prototype-retirement-failed');
    try {
      rollback(outcome.output);
    } catch (cleanup) {
      throw new AggregateError(
        [primary, cleanup],
        'reorder-paint-output-transfer-retirement-failed',
      );
    }
    throw primary;
  }
  return outcome.output;
}
