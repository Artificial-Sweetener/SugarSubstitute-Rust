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
/** One finite root-bound stylesheet retains bounded prepared rules under the exact current mask and explicit hover policy. */
import { nativeReorderLineClass } from './reorder-native-targets.js';
import type { EditorView } from '@codemirror/view';
import { reorderPaintIssueName } from './reorder-paint-identity.js';
/** Existing pointer ownership selects held drag paint; idle keyboard motion retains native hover transfer. */
export type ReorderMotionHoverPolicy = 'native' | 'held' | 'whole';
/** One immutable artifact retains the bounded roster, explicit policy and final emitted rule bytes. */
interface PreparedMotionRules {
  readonly ids: readonly number[];
  readonly hover: ReorderMotionHoverPolicy;
  readonly cssText: string;
}
/** Stable CSS scope is separate from exact per-adoption mask and queued-release authority. */
export class ReorderMotionStyles {
  readonly scope = Object.freeze({});
  readonly root: Document | ShadowRoot;
  private readonly document: Document;
  private node: HTMLStyleElement | null = null;
  private marker: HTMLSpanElement | null = null;
  private markerReady = false;
  private readonly editor: HTMLElement;
  private prepared: PreparedMotionRules | null = null;
  private active: object | null = null;
  private generation = 0;
  private retired = false;
  /** CodeMirror's supported roots are native Document or ShadowRoot; no stylesheet is acquired before use. */
  constructor(view: EditorView) {
    this.document = view.dom.ownerDocument;
    this.editor = view.dom;
    this.root = view.root as Document | ShadowRoot;
  }
  /** A failed or cancelled owner cannot be reused by a later adoption. */
  get available(): boolean {
    return !this.retired;
  }
  /** Recheck exact ownership after the caller's potentially reentrant admission callback. */
  private admitted(
    generation: number,
    node: HTMLStyleElement | null,
    current: () => boolean,
    marker = this.marker,
  ): boolean {
    if (
      this.retired ||
      generation !== this.generation ||
      this.node !== node ||
      this.marker !== marker
    )
      return false;
    const accepted = current();
    return (
      accepted &&
      !this.retired &&
      generation === this.generation &&
      this.node === node &&
      this.marker === marker
    );
  }
  /** Native attachment is checked after delegated admission and fenced again after every native accessor. */
  private mounted(
    generation: number,
    node: HTMLStyleElement,
    marker: HTMLSpanElement,
    issue: object | null,
    current: () => boolean,
  ): boolean {
    if (
      !this.markerReady ||
      this.active !== issue ||
      !this.admitted(generation, node, current, marker)
    )
      return false;
    const attached =
      marker.parentElement === this.editor &&
      this.editor.childNodes[0] === marker &&
      marker.isConnected &&
      marker.getRootNode() === this.root;
    return (
      attached &&
      !this.retired &&
      generation === this.generation &&
      this.node === node &&
      this.marker === marker &&
      this.markerReady &&
      this.active === issue
    );
  }
  /** The finite prepared roster shares one validation and bounded-union policy across ordinary and whole transfers. */
  private roster(ids: readonly number[]): number[] {
    if (
      ids.length > 64 ||
      new Set(ids).size !== ids.length ||
      ids.some((id) => !Number.isSafeInteger(id) || id < 0)
    )
      throw new TypeError('reorder-motion-style-roster-invalid');
    const union = new Set([...(this.prepared?.ids ?? []), ...ids]);
    return [...(union.size <= 64 ? union : ids)].sort((a, b) => a - b);
  }
  /** A whole-value rule ignores IDs; transfer only JavaScript issue authority after exact retained physical admission. */
  transfer(
    previous: object,
    issue: object,
    ids: readonly number[],
    current: () => boolean,
  ): boolean {
    const node = this.node,
      marker = this.marker,
      prepared = this.prepared,
      generation = ++this.generation;
    if (!node || !marker || prepared?.hover !== 'whole' || this.active !== previous) return false;
    const roster = this.roster(ids),
      scope = reorderPaintIssueName(this.scope);
    /** A delegated getter cannot certify a replaced sheet, policy or publication. */
    const admitted = (): boolean =>
      this.prepared === prepared &&
      this.active === previous &&
      this.mounted(generation, node, marker, previous, current) &&
      this.prepared === prepared &&
      this.active === previous;
    if (!admitted()) return false;
    for (const read of [
      () => node.isConnected,
      () => node.parentNode === ('head' in this.root ? this.root.head : this.root),
      () => node.getRootNode() === this.root,
      () => node.textContent === prepared.cssText,
      () => marker.getAttribute('data-reorder-motion-gate') === scope,
      () => marker.getAttribute('data-reorder-motion-active') === scope,
    ]) {
      const accepted = read();
      if (!admitted() || !accepted) return false;
    }
    this.prepared = Object.freeze({
      ids: Object.freeze(roster),
      hover: 'whole',
      cssText: prepared.cssText,
    });
    this.active = issue;
    return true;
  }
  /** Prepare bounded rule bytes while the local marker is inactive; selected subsets reuse the actual sheet and its union. */
  prepare(
    ids: readonly number[],
    hover: ReorderMotionHoverPolicy,
    current: () => boolean,
  ): boolean {
    const generation = ++this.generation;
    if (!this.admitted(generation, this.node, current)) return false;
    if (this.active !== null) throw new Error('reorder-motion-style-still-active');
    const roster = this.roster(ids);
    let node = this.node;
    try {
      if (!node) {
        node = this.document.createElement('style');
        if (!this.admitted(generation, null, current)) {
          node.remove();
          return false;
        }
        this.node = node;
      }
      if (!this.admitted(generation, node, current)) return false;
      if (this.marker && !this.markerReady) {
        const partial = this.marker;
        this.marker = null;
        partial.remove();
        if (!this.admitted(generation, node, current)) return false;
      }
      if (!this.marker) {
        const marker = this.document.createElement('span');
        if (!this.admitted(generation, node, current)) {
          marker.remove();
          return false;
        }
        this.marker = marker;
        this.markerReady = false;
        marker.style.setProperty('display', 'none', 'important');
        if (!this.admitted(generation, node, current)) return false;
        marker.hidden = true;
        if (!this.admitted(generation, node, current)) return false;
        marker.inert = true;
        if (!this.admitted(generation, node, current)) return false;
        marker.setAttribute('aria-hidden', 'true');
        if (!this.admitted(generation, node, current)) return false;
        marker.setAttribute('data-reorder-motion-gate', reorderPaintIssueName(this.scope));
        if (!this.admitted(generation, node, current)) return false;
        this.markerReady = true;
      }
      const marker = this.marker;
      const scope = reorderPaintIssueName(this.scope),
        gate = `> [data-reorder-motion-gate="${scope}"][data-reorder-motion-active="${scope}"] ~`;
      const nativeValue = this.editor.getAttribute('data-reorder-native-value');
      if (!this.admitted(generation, node, current)) return false;
      if (nativeValue !== null && !/^(0|[1-9][0-9]*)$/u.test(nativeValue))
        throw new Error('reorder-motion-native-value-invalid');
      const nativeSelectors =
        nativeValue === null
          ? []
          : ids.map(
              (id) =>
                '[data-reorder-native-value="' +
                nativeValue +
                '"][data-reorder-chip="' +
                String(id) +
                '"]',
            );
      const editor =
        nativeValue === null
          ? '.cm-editor'
          : '.cm-editor[data-reorder-native-value="' +
            nativeValue +
            '"][data-reorder-motion-scope="' +
            scope +
            '"]';
      const underlay =
        nativeValue === null
          ? '.sp-reorder-pointer-underlay'
          : '.' + nativeReorderLineClass(nativeValue);
      const local = nativeSelectors.length
          ? ':is([data-reorder-motion-local="' + scope + '"],' + nativeSelectors.join(',') + ')'
          : '[data-reorder-motion-local="' + scope + '"]',
        cssText =
          hover === 'whole'
            ? `${editor} ${gate} .cm-scroller > .cm-content ${underlay}{opacity:0!important}`
            : hover === 'held'
              ? `${editor} ${gate} .cm-scroller > .cm-content ${local}[data-reorder-chip]{opacity:0!important}`
              : `${editor} ${gate} .cm-scroller > .cm-content ${local}[data-reorder-chip]:not(:hover){opacity:0!important}\n` +
                roster
                  .map(
                    (id) =>
                      `${editor} ${gate} .cm-scroller:has(> .cm-content ${local}[data-reorder-chip="${id}"]:hover) > .sp-reorder-motion-layer > .sp-reorder-motion-mount > .sp-reorder-motion-stage .sp-reorder-motion-paint [data-reorder-motion-chip="${id}"]{visibility:hidden!important}`,
                  )
                  .join('\n');
      if (this.prepared?.cssText !== cssText) {
        this.prepared = null;
        node.textContent = cssText;
        if (!this.admitted(generation, node, current)) return false;
      }
      this.prepared = Object.freeze({
        ids: Object.freeze(roster),
        hover,
        cssText,
      });
      if (!node.isConnected) {
        if ('head' in this.root) this.root.head.append(node);
        else this.root.append(node);
      }
      if (!this.admitted(generation, node, current)) return false;
      // Move only this owned hidden marker; existing managed children and the external stage retain their relative order.
      if (this.editor.childNodes[0] !== marker)
        this.editor.insertBefore(marker, this.editor.childNodes[0] ?? null);
      return this.mounted(generation, node, marker, null, current);
    } catch (primary) {
      if (!this.retired && generation === this.generation) {
        try {
          this.retire();
        } catch (cleanup) {
          throw new AggregateError([primary, cleanup], 'reorder-motion-style-preparation-failed');
        }
      }
      throw primary;
    }
  }
  /** Activate only after the caller admits the exact native mask issue; reentry cannot activate an older issue. */
  activate(issue: object, current: () => boolean): boolean {
    const node = this.node,
      marker = this.marker,
      generation = ++this.generation;
    try {
      if (
        !node ||
        !marker ||
        this.prepared === null ||
        !this.mounted(generation, node, marker, null, current)
      )
        return false;
      this.active = issue;
      marker.setAttribute('data-reorder-motion-active', reorderPaintIssueName(this.scope));
      if (!this.mounted(generation, node, marker, issue, current)) {
        this.deactivate(issue);
        return false;
      }
      return true;
    } catch (primary) {
      if (
        generation === this.generation &&
        this.active === issue &&
        this.node === node &&
        this.marker === marker
      ) {
        try {
          this.retire();
        } catch (cleanup) {
          throw new AggregateError([primary, cleanup], 'reorder-motion-style-activation-failed');
        }
      }
      throw primary;
    }
  }
  /** Synchronously reveal native paint before its exact scene retires; an old issue cannot disable a newer one. */
  deactivate(issue: object): void {
    if (this.active !== issue) return;
    const node = this.node,
      marker = this.marker,
      generation = ++this.generation;
    this.active = null;
    if (!node) return;
    try {
      marker?.setAttribute('data-reorder-motion-active', '');
    } catch (primary) {
      if (
        generation === this.generation &&
        this.node === node &&
        this.marker === marker &&
        this.active === null
      ) {
        try {
          this.retire();
        } catch (cleanup) {
          throw new AggregateError([primary, cleanup], 'reorder-motion-style-deactivation-failed');
        }
      }
      throw primary;
    }
  }
  /** Clear references before native callbacks and independently attempt deactivation and physical sheet removal. */
  retire(): void {
    this.retired = true;
    ++this.generation;
    const node = this.node,
      marker = this.marker;
    this.node = null;
    this.marker = null;
    this.markerReady = false;
    this.prepared = null;
    this.active = null;
    if (!node && !marker) return;
    const failures: unknown[] = [];
    for (const release of [
      () => {
        marker?.setAttribute('data-reorder-motion-active', '');
      },
      () => marker?.remove(),
      () => node?.remove(),
    ])
      try {
        release();
      } catch (cause) {
        failures.push(cause);
      }
    if (failures.length === 1) throw failures[0];
    if (failures.length)
      throw new AggregateError(failures, 'reorder-motion-style-retirement-failed');
  }
}
