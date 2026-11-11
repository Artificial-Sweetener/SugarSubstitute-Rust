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
/** Controlled native popover transport exercises the actual shared owner without certifying CSS or layout. */
import { Compartment, EditorState } from '@codemirror/state';
import { showTooltip, type EditorView, type ViewUpdate } from '@codemirror/view';
import { NativeTooltipParent } from '../../../../../../frontend/sugarprompt/editor/src/tooltip-parent.ts';

/** Literal DOM boundary retains only operations and independently supplied native outcomes. */
class PopoverNode extends EventTarget {
  readonly style: Record<string, string> = {};
  readonly listeners = new Map<string, Set<EventListenerOrEventListenerObject>>();
  readonly attributes = new Map<string, string>();
  readonly children: PopoverNode[] = [];
  className = '';
  popover: string | null = null;
  textContent = '';
  connected = false;
  open = false;
  shows = 0;
  hides = 0;
  removals = 0;
  checks = 0;
  cancel = false;
  beforeConnected: (() => void) | null = null;
  beforeShow: (() => void) | null = null;
  beforeHide: (() => void) | null = null;
  appendFailure: { readonly cause: unknown } | null = null;
  showFailure: { readonly cause: unknown } | null = null;
  hideFailure: { readonly cause: unknown } | null = null;
  removeFailure: { readonly cause: unknown } | null = null;
  readonly ownerDocument: { createElement: (tag: string) => PopoverNode };
  /** Every literal node retains its creating native document. */
  constructor(document: PopoverNode['ownerDocument']) {
    super();
    this.ownerDocument = document;
  }
  /** Native connection is an external structural operand, not layout or CSS. */
  get isConnected(): boolean {
    const connected = this.connected,
      hook = this.beforeConnected;
    this.beforeConnected = null;
    hook?.();
    return connected;
  }
  /** Track real EventTarget listener acquisition without reproducing publication policy. */
  override addEventListener(
    type: string,
    callback: EventListenerOrEventListenerObject | null,
    options?: AddEventListenerOptions | boolean,
  ): void {
    super.addEventListener(type, callback, options);
    if (callback) {
      const entries = this.listeners.get(type) ?? new Set<EventListenerOrEventListenerObject>();
      entries.add(callback);
      this.listeners.set(type, entries);
    }
  }
  /** Release real EventTarget callbacks and their independently observable retention. */
  override removeEventListener(
    type: string,
    callback: EventListenerOrEventListenerObject | null,
    options?: EventListenerOptions | boolean,
  ): void {
    super.removeEventListener(type, callback, options);
    if (callback) {
      const entries = this.listeners.get(type);
      entries?.delete(callback);
      if (entries?.size === 0) this.listeners.delete(type);
    }
  }
  /** Record literal landmark metadata without accessibility emulation. */
  setAttribute(name: string, value: string): void {
    this.attributes.set(name, value);
  }
  /** Retain owned child identities; geometry and CSS are deliberately absent. */
  append(child: PopoverNode): void {
    this.children.push(child);
    child.connected = true;
    if (this.appendFailure) throw this.appendFailure.cause;
  }
  /** Closed native selector state is supplied by this external boundary, not a second publication policy. */
  matches(selector: string): boolean {
    if (selector !== ':popover-open') throw new Error('UNEXPECTED_POPOVER_SELECTOR');
    this.checks++;
    return this.open;
  }
  /** A controlled beforetoggle hook can cancel, fail, replace or dispose during acquisition. */
  showPopover(): void {
    this.shows++;
    this.beforeShow?.();
    if (this.showFailure) throw this.showFailure.cause;
    if (!this.connected) throw new Error('DISCONNECTED_POPOVER_BOUNDARY');
    if (!this.cancel) this.open = true;
  }
  /** A controlled native hide retains independent failure identity. */
  hidePopover(): void {
    this.hides++;
    this.beforeHide?.();
    if (this.hideFailure) throw this.hideFailure.cause;
    this.open = false;
  }
  /** Removal independently retires native top-layer state, including an in-progress show/hide. */
  remove(): void {
    this.removals++;
    if (this.removeFailure) throw this.removeFailure.cause;
    this.connected = false;
    this.open = false;
  }
}

/** One fixture owns literal DOM, actual facet state, real parent/lease callbacks and the finite microtask transport. */
export function tooltipParentFixture(
  options: {
    readonly mountFailure?: { readonly cause: unknown };
    readonly removeFailure?: { readonly cause: unknown };
  } = {},
) {
  const nodes: PopoverNode[] = [],
    jobs: VoidFunction[] = [];
  const document = {
    /** Create only literal nodes; tag identity remains available to assertions. */
    createElement(_tag: string): PopoverNode {
      const node = new PopoverNode(document);
      if (nodes.length === 0) node.removeFailure = options.removeFailure ?? null;
      nodes.push(node);
      return node;
    },
  };
  const host = new PopoverNode(document),
    original = globalThis.queueMicrotask;
  host.appendFailure = options.mountFailure ?? null;
  globalThis.queueMicrotask = (callback) => {
    jobs.push(callback);
  };
  let owner: NativeTooltipParent;
  try {
    owner = new NativeTooltipParent(host as unknown as HTMLElement, 'Prompt text', (plane) => {
      const node = plane as unknown as PopoverNode;
      node.popover = 'manual';
      return {
        show: () => node.showPopover(),
        hide: () => node.hidePopover(),
        dispose: () => undefined,
      };
    });
  } catch (cause) {
    globalThis.queueMicrotask = original;
    throw cause;
  }
  const root = nodes[0]!;
  return {
    owner,
    host,
    root,
    jobs,
    /** Bind the production plugin callback path to real supported CodeMirror facet values. */
    bind(active = false, beforeFacet?: () => void) {
      const slot = new Compartment();
      /** The unit never creates tooltip content; native browser proof owns actual child mounting. */
      const tooltip = {
        pos: 0,
        create: () => ({ dom: root as unknown as HTMLElement }),
      };
      let state = EditorState.create({
          extensions: slot.of(showTooltip.of(active ? tooltip : null)),
        }),
        reject = false;
      const view = {
        get state() {
          if (reject) throw new Error('RETIRED_TOOLTIP_VIEW_READ');
          return state;
        },
      } as unknown as EditorView;
      if (beforeFacet) {
        const read = state.facet;
        let pending = true;
        Object.defineProperty(state, 'facet', {
          /** Keep actual immutable facet evaluation; only this external getter can reenter once. */
          get() {
            if (pending) {
              pending = false;
              beforeFacet();
            }
            return read;
          },
        });
      }
      const lease = owner.bind(view);
      return {
        lease,
        /** Replace only the native facet input; the actual owner decides publication. */
        update(next: boolean): void {
          const previous = state;
          state = state.update({
            effects: slot.reconfigure(showTooltip.of(next ? tooltip : null)),
          }).state;
          lease.update({ startState: previous, state } as ViewUpdate);
        },
        /** A superseded view transport must not be sampled by an old callback. */
        rejectReads(): void {
          reject = true;
        },
      };
    },
    /** Deliver one explicitly retained callback, never an arbitrary settling loop. */
    flush(): void {
      jobs.shift()!();
    },
    /** Restore the process transport even if native teardown rejects. */
    destroy(): void {
      try {
        owner.dispose();
      } finally {
        globalThis.queueMicrotask = original;
      }
    },
  };
}
