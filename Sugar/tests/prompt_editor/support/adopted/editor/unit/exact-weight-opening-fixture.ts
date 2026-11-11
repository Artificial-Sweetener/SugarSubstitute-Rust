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
/** Controlled DOM and measurement transport exercises actual opening, focus and command owners. */
import { readFileSync } from 'node:fs';
import { registerHooks } from 'node:module';
import {
  EditorState,
  Transaction,
  type TransactionSpec,
  type StateEffectType,
} from '@codemirror/state';
import { EditorView, type ViewUpdate } from '@codemirror/view';
import {
  NativeSurface,
  type NativeViewFactory,
} from '../../../../../../frontend/sugarprompt/editor/src/native-surface.ts';
import { exactWeightDraft } from '../../../../../../frontend/sugarprompt/editor/src/exact-weight-draft.ts';
import { historyTransactions } from '../../../../../../frontend/sugarprompt/editor/src/prompt-history.ts';
import { weightStateFor } from '../transaction/weight-command-state.ts';
/** Only public production methods are observed through an in-memory class export. */
interface Opening {
  press(event: MouseEvent): boolean;
  mount(key: symbol, cell: HTMLElement, value: string): { release(): void };
  update(update: ViewUpdate): void;
  destroy(): void;
}
/** The genuine queue retains all source preparation and publication responsibility. */
interface Queue {
  beginExact: (value: unknown) => void;
  exact: (...args: unknown[]) => boolean;
  cancelTarget: (owner: unknown) => void;
  update(update: ViewUpdate): void;
  destroy(): void;
}
const exports = new Map([
  [
    new URL('../../../../../../frontend/sugarprompt/editor/src/exact-weight.ts', import.meta.url)
      .href,
    'ExactWeights',
  ],
  [
    new URL(
      '../../../../../../frontend/sugarprompt/editor/src/weight-interactions.ts',
      import.meta.url,
    ).href,
    'WeightCommands',
  ],
]);
const hook = registerHooks({
  /** Preserve the complete production source and expose its class only in this process. */
  load(url, context, next) {
    const result = next(url, context),
      name = exports.get(url);
    return name
      ? {
          ...result,
          source: readFileSync(new URL(url), 'utf8') + `\nexport { ${name} };\n`,
        }
      : result;
  },
});
const openingModule = (await import(
  '../../../../../../frontend/sugarprompt/editor/src/exact-weight.ts'
)) as unknown as {
  ExactWeights: new (view: EditorView) => Opening;
};
const queueModule = (await import(
  '../../../../../../frontend/sugarprompt/editor/src/weight-interactions.ts'
)) as unknown as {
  WeightCommands: new (view: EditorView) => Queue;
};
hook.deregister();
/** Build one external DOM adapter; native input listeners and AbortController remain actual owners. */
export function openingControl() {
  const globals = {
    Element: globalThis.Element,
    document: globalThis.document,
    window: globalThis.window,
  };
  const root: { activeElement: object | null } = { activeElement: null };
  /** Literal DOM primitives support the actual input constructor and selector boundary. */
  class Node extends EventTarget {
    style = {};
    dataset: Record<string, string> = {};
    textContent = '';
    className = '';
    value = '';
    type = '';
    inputMode = '';
    autocomplete = '';
    spellcheck = false;
    selectionStart = 0;
    selectionEnd = 0;
    isConnected = true;
    children: Node[] = [];
    /** Attribute transport has no semantic owner. */
    setAttribute(): void {}
    /** Detached subtree construction belongs to the actual input owner. */
    append(...nodes: Node[]): void {
      this.children.push(...nodes);
    }
    /** Match only the independent label/input specimen. */
    closest(selector: string): Node | null {
      return selector === '.sp-exact-weight-input'
        ? this.className === 'sp-exact-weight-input'
          ? this
          : null
        : this === label
          ? label
          : null;
    }
    /** The literal specimen is an emphasis suffix. */
    matches(): boolean {
      return false;
    }
    /** Record actual focus ownership without granting native browser authority. */
    focus(): void {
      root.activeElement = this;
    }
    /** The genuine input invokes its platform select operation exactly once. */
    select(): void {
      this.selectionStart = 0;
      this.selectionEnd = this.value.length;
    }
  }
  const content = new Node(),
    cell = new Node(),
    label = new Node(),
    body = new Node();
  label.textContent = '1.20';
  const window = new EventTarget();
  const tasks = new Map<number, () => void>();
  let nextTask = 0;
  Object.assign(window, {
    setTimeout(callback: () => void) {
      tasks.set(++nextTask, callback);
      return nextTask;
    },
    clearTimeout(handle: number) {
      tasks.delete(handle);
    },
  });
  Object.assign(globalThis, {
    Element: Node,
    document: { createElement: () => new Node() },
    window,
  });
  let state!: EditorState,
    mounted: { release(): void } | null = null,
    mountedKey: symbol | null = null;
  const measures: {
    read(view: EditorView): unknown;
    write(value: unknown, view: EditorView): void;
  }[] = [];
  let reveals = 0;
  /** Pinned CM effect identity is observed only to count actual reveal requests at this external transport boundary. */
  const revealType = (
    EditorView.scrollIntoView(0) as unknown as {
      readonly type: StateEffectType<unknown>;
    }
  ).type;
  const factory: NativeViewFactory = (_parent, initial) => {
    state = initial;
    const view = {
      root,
      contentDOM: content,
      dom: {
        contains: (node: object) =>
          node === content || node === cell || cell.children.includes(node as Node),
      },
      scrollDOM: { clientHeight: 50 },
      get state() {
        return state;
      },
      get hasFocus() {
        return root.activeElement === content;
      },
      composing: false,
      compositionStarted: false,
      plugin: () => queue,
      posAtDOM: () => 4,
      focus() {
        root.activeElement = content;
      },
      scrollSnapshot: () => EditorView.scrollIntoView(state.selection.main.head),
      /** Publish complete genuine transactions and invoke current owner updates. */
      dispatch(input: readonly Transaction[] | Transaction | TransactionSpec) {
        const before = state;
        const transactions: readonly Transaction[] = Array.isArray(input)
          ? input
          : [input instanceof Transaction ? input : state.update(input as TransactionSpec)];
        for (const transaction of historyTransactions(transactions)) {
          state = transaction.state;
          reveals += transaction.effects.filter((effect) => effect.is(revealType)).length;
        }
        const update = { startState: before, state } as ViewUpdate;
        queue?.update(update);
        opening?.update(update);
      },
      requestMeasure(request: unknown) {
        measures.push(request as (typeof measures)[number]);
      },
      destroy() {},
      setState(next: EditorState) {
        state = next;
      },
    };
    return view as unknown as EditorView;
  };
  const surface = new NativeSurface(
    { ownerDocument: { body } } as unknown as HTMLElement,
    weightStateFor('(cat:1.20), tail', 15),
    () => {},
    factory,
  );
  const queue = new queueModule.WeightCommands(surface.view);
  const opening = new openingModule.ExactWeights(surface.view);
  return {
    surface,
    get state() {
      return state;
    },
    get reveals() {
      return reveals;
    },
    /** Invoke the real supported numeric activation before mounting its actual keyed input. */
    open() {
      const event = new Event('mousedown', { cancelable: true });
      Object.assign(event, {
        button: 0,
        ctrlKey: false,
        altKey: false,
        metaKey: false,
        detail: 2,
      });
      Object.defineProperty(event, 'target', { value: label });
      const accepted = opening.press(event as MouseEvent),
        draft = state.field(exactWeightDraft);
      if (!draft) throw new Error('Actual opening draft missing');
      mounted?.release();
      cell.children = [];
      mounted = opening.mount(draft.key, cell as unknown as HTMLElement, '1.20');
      mountedKey = draft.key;
      return accepted;
    },
    get draftKey() {
      return mountedKey;
    },
    /** Deliver the input's actual key callback, without claiming a trusted native event. */
    key(key: string, value = '2.40') {
      const input = cell.children.find((node) => node.className === 'sp-exact-weight-input');
      if (!input) throw new Error('Actual input missing');
      input.value = value;
      const event = new Event('keydown', { cancelable: true });
      Object.assign(event, { key, isComposing: false });
      input.dispatchEvent(event);
    },
    /** Deliver one existing measurement and its actual write microtask. */
    async measure() {
      const job = measures.shift();
      if (!job) throw new Error('Measurement missing');
      const view = surface.view;
      job.write(job.read(view), view);
      await Promise.resolve();
    },
    /** Drain only already owned work, bounded independently of implementation. */
    async drain() {
      for (let count = 0; measures.length || tasks.size; count++) {
        if (count > 30) throw new Error('Control work exceeded');
        for (const [id, callback] of tasks) {
          tasks.delete(id);
          callback();
        }
        if (measures.length) await this.measure();
      }
      await Promise.resolve();
    },
    /** Eviction runs the real mount release rather than publishing an invented state effect. */
    evict() {
      mounted?.release();
      mounted = null;
    },
    /** Actual source replacement invokes owner freshness retirement. */
    replace() {
      surface.view.dispatch({
        changes: { from: 0, to: state.doc.length, insert: 'other' },
      });
    },
    /** Interaction disposal must not retire a newer host focus request. */
    destroyOpening() {
      opening.destroy();
    },
    /** Teardown releases every actual timer/listener/capability before restoring external globals. */
    dispose() {
      opening.destroy();
      queue.destroy();
      surface.dispose();
      Object.assign(globalThis, globals);
    },
  };
}
