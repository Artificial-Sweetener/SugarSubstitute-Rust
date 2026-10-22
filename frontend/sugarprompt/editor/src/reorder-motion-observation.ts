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
/** One acquisition owns native mutation subscriptions and their exact pending-record policy. */
import type { EditorView } from '@codemirror/view';
import { isReorderDerivedRootAttribute } from './reorder-derived-root-attribute.js';
import type { NativeReorderCapture } from './reorder-command.js';
import type { ReorderPreview } from './reorder-preview-contracts.js';
import { reorderPreview } from './reorder-preview.js';

/** Resource delegates freshness and invalidation without exposing Scene, mask or handoff publication. */
interface ReorderMotionObservationPorts {
  owned(): boolean;
  current(): boolean;
  observe<T>(action: () => T): T;
  changed(): void;
}

/** Native observer state belongs to exactly one acquisition, independently of its paint resources. */
export class ReorderMotionObservation {
  private readonly view: EditorView;
  private readonly capture: NativeReorderCapture;
  private readonly ports: ReorderMotionObservationPorts;
  private publication: ReorderPreview | null;
  private content: MutationObserver | null = null;
  private root: MutationObserver | null = null;
  private started = false;
  private disposed = false;

  /** Allocate no native subscriptions until Resource has installed this handle on its acquisition. */
  constructor(
    view: EditorView,
    capture: NativeReorderCapture,
    ports: ReorderMotionObservationPorts,
  ) {
    this.view = view;
    this.capture = capture;
    this.ports = ports;
    this.publication = view.state.field(reorderPreview, false)?.value ?? null;
  }

  /** Subscribe once; each native handle is retained before any observe callback can reenter. */
  start(): void {
    if (this.started || this.disposed)
      throw new Error('reorder-motion-observation-already-started-or-retired');
    this.started = true;
    const Observer = this.view.dom.ownerDocument.defaultView?.MutationObserver;
    if (!Observer || this.disposed) return;
    const content = new Observer(() =>
      this.ports.observe(() => {
        if (this.disposed || !this.ports.owned()) return;
        const publication = this.view.state.field(reorderPreview, false)?.value ?? null;
        const readonlyIssue =
          publication?.capture === this.capture && publication !== this.publication;
        this.publication = publication;
        if (!this.ports.current() || !readonlyIssue) this.ports.changed();
      }),
    );
    this.content = content;
    if (this.disposed) {
      this.content = null;
      content.disconnect();
      return;
    }
    const root = new Observer((records) =>
      this.ports.observe(() => {
        if (this.disposed || !this.ports.owned()) return;
        if (records.length > 64) {
          this.ports.changed();
          return;
        }
        for (const record of records) {
          if (!isReorderDerivedRootAttribute(record.attributeName)) {
            if (!this.ports.current()) this.ports.changed();
            return;
          }
        }
      }),
    );
    this.root = root;
    if (this.disposed) {
      this.root = null;
      root.disconnect();
      return;
    }
    this.connect(root, this.view.dom, { attributes: true });
    if (this.disposed) return;
    this.connect(content, this.view.contentDOM, {
      childList: true,
      subtree: true,
      characterData: true,
      attributes: true,
      attributeOldValue: true,
    });
  }

  /** A retiring native observe may complete after disconnect; settle only that captured subscription and preserve both raw causes. */
  private connect(observer: MutationObserver, target: Node, options: MutationObserverInit): void {
    let failure: { readonly cause: unknown } | null = null;
    try {
      observer.observe(target, options);
    } catch (cause) {
      failure = { cause };
    }
    if (this.disposed)
      try {
        observer.disconnect();
      } catch (cleanup) {
        if (failure)
          throw new AggregateError(
            [failure.cause, cleanup],
            'reorder-motion-observation-subscription-retirement-failed',
          );
        throw cleanup;
      }
    if (failure) throw failure.cause;
  }

  /** Preserve native content records verbatim for the independent visibility receipt; absent native observation stays null. */
  takeContentRecords(): readonly MutationRecord[] | null {
    return this.content?.takeRecords() ?? null;
  }

  /** Drain foreign content before bounded root metadata, fencing the caller's synchronous acquisition after each native read. */
  cleanPending(available: () => boolean): boolean {
    if (this.disposed) return false;
    const records = this.content?.takeRecords() ?? [];
    if (!available() || records.length) return false;
    const roots = this.root?.takeRecords() ?? [];
    return (
      available() &&
      roots.length <= 64 &&
      roots.every(
        (record) =>
          record.type === 'attributes' && isReorderDerivedRootAttribute(record.attributeName),
      )
    );
  }

  /** Revoke callbacks before independently disconnecting content then root; Resource preserves these raw causes in its cleanup order. */
  disconnect(): readonly unknown[] {
    if (this.disposed) return [];
    this.disposed = true;
    const content = this.content,
      root = this.root,
      failures: unknown[] = [];
    this.content = null;
    this.root = null;
    this.publication = null;
    for (const observer of [content, root])
      try {
        observer?.disconnect();
      } catch (cause) {
        failures.push(cause);
      }
    return failures;
  }
}
