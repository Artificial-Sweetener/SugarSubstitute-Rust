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
/** Literal native resize and public plugin-registration transports exercise the actual layout owner without a browser. */
import assert from 'node:assert/strict';
import { ViewPlugin, type EditorView, type ViewUpdate } from '@codemirror/view';
import { nativeLayout } from '../../../../../../frontend/sugarprompt/editor/src/native-layout.ts';
import type { EditorLayoutMetrics } from '../../../../../../frontend/sugarprompt/editor/src/contracts.ts';

/** Only the plugin's supported finite lifecycle crosses the registration boundary. */
interface LayoutOwner {
  update(update: ViewUpdate): void;
  destroy(): void;
}
/** Raw failures remain presence-tagged so null and undefined are genuine causes. */
export interface LayoutFaults {
  readonly construction?: { readonly cause: unknown };
  readonly observe?: { readonly at: number; readonly cause: unknown };
  readonly disconnect?: { readonly cause: unknown };
  readonly request?: { readonly cause: unknown };
}
/** Capture the actual public registration factory and restore its complete original descriptor before constructing the owner. */
function createOwner(
  view: EditorView,
  observe: ((value: EditorLayoutMetrics) => void) | undefined,
): LayoutOwner {
  const descriptor = Object.getOwnPropertyDescriptor(ViewPlugin, 'define');
  assert.ok(descriptor);
  const define = ViewPlugin.define;
  let factory: unknown;
  try {
    Object.defineProperty(ViewPlugin, 'define', {
      ...descriptor,
      value(...args: unknown[]) {
        factory = args[0];
        return Reflect.apply(define, ViewPlugin, args);
      },
    });
    nativeLayout(observe, () => 0);
  } finally {
    Object.defineProperty(ViewPlugin, 'define', descriptor);
  }
  assert.equal(typeof factory, 'function');
  const value: unknown = Reflect.apply(factory as (...args: unknown[]) => unknown, undefined, [
    view,
    undefined,
  ]);
  assert.ok(value && typeof value === 'object');
  assert.equal(typeof Reflect.get(value, 'update'), 'function');
  assert.equal(typeof Reflect.get(value, 'destroy'), 'function');
  return value as LayoutOwner;
}

/** This finite platform transport never calculates layout, scheduling policy or publication equality. */
export function nativeLayoutFixture(faults: LayoutFaults = {}) {
  const descriptor = Object.getOwnPropertyDescriptor(globalThis, 'ResizeObserver'),
    requests: { read(): unknown; write(value: unknown): void }[] = [],
    published: EditorLayoutMetrics[] = [],
    instances: ResizeBoundary[] = [];
  const geometry = { content: 33, line: 20, viewport: 33, scale: 1 };
  let reading = false,
    reads = 0,
    owner: LayoutOwner | null = null,
    onPublish: (value: EditorLayoutMetrics) => void = () => {};
  /** Native callback delivery is explicit; observe/disconnect only transport target ownership. */
  class ResizeBoundary implements ResizeObserver {
    readonly targets = new Set<Element>();
    readonly callback: ResizeObserverCallback;
    observes = 0;
    disconnects = 0;
    constructor(callback: ResizeObserverCallback) {
      if (faults.construction) throw faults.construction.cause;
      this.callback = callback;
      instances.push(this);
    }
    observe(target: Element): void {
      this.targets.add(target);
      if (++this.observes === faults.observe?.at) throw faults.observe.cause;
    }
    unobserve(target: Element): void {
      this.targets.delete(target);
    }
    disconnect(): void {
      this.disconnects++;
      this.targets.clear();
      if (faults.disconnect) throw faults.disconnect.cause;
    }
  }
  /** Every geometry getter must be called inside the real owner's requested READ. */
  function measured(value: number): number {
    assert.equal(reading, true);
    reads++;
    return value;
  }
  const parent = {},
    document = {
      defaultView: {
        getComputedStyle() {
          assert.equal(reading, true);
          reads++;
          return {
            borderTopWidth: '0',
            borderBottomWidth: '0',
            paddingTop: '0',
            paddingBottom: '0',
          };
        },
      },
    },
    dom = { parentElement: parent, ownerDocument: document },
    contentDOM = {};
  // This exact external view seam transports public measurement fields; no EditorView, renderer or layout algorithm is simulated.
  const view = {
    dom,
    contentDOM,
    get contentHeight() {
      return measured(geometry.content * geometry.scale);
    },
    get defaultLineHeight() {
      return measured(geometry.line * geometry.scale);
    },
    get scaleY() {
      return measured(geometry.scale);
    },
    get documentPadding() {
      assert.equal(reading, true);
      return { top: 0, bottom: 0 };
    },
    scrollDOM: {
      get clientHeight() {
        return measured(geometry.viewport);
      },
    },
    requestMeasure(request: unknown) {
      if (faults.request) throw faults.request.cause;
      assert.ok(request && typeof request === 'object');
      const read: unknown = Reflect.get(request, 'read'),
        write: unknown = Reflect.get(request, 'write');
      assert.equal(typeof read, 'function');
      assert.equal(typeof write, 'function');
      requests.push({
        read: () => Reflect.apply(read as (...args: unknown[]) => unknown, undefined, [view]),
        write: (value) => {
          Reflect.apply(write as (...args: unknown[]) => unknown, undefined, [value, view]);
        },
      });
    },
  } as unknown as EditorView;
  Object.defineProperty(globalThis, 'ResizeObserver', {
    configurable: true,
    writable: true,
    value: ResizeBoundary,
  });
  return {
    view,
    geometry,
    requests,
    published,
    instances,
    /** Initial acquisition is explicit so constructor failures still leave fixture cleanup owned by the test. */
    create(observed = true): void {
      owner = createOwner(
        view,
        observed
          ? (value) => {
              published.push(value);
              onPublish(value);
            }
          : undefined,
      );
    },
    /** Host size delivery does not synthesize a CodeMirror update or calculate native geometry. */
    resize(): void {
      for (const instance of instances) if (instance.targets.size) instance.callback([], instance);
    },
    /** A callback already captured by the platform may arrive after disconnect; the actual owner must reject it. */
    lateResize(): void {
      for (const instance of instances) instance.callback([], instance);
    },
    /** Deliver the actual queued owner's READ independently from its WRITE. */
    read() {
      const request = requests.shift();
      assert.ok(request);
      reading = true;
      try {
        return { request, value: request.read() };
      } finally {
        reading = false;
      }
    },
    /** Native work is delivered only by this finite transport; it never creates another measurement request. */
    flush(): void {
      const request = this.read();
      request.request.write(request.value);
    },
    get reads() {
      return reads;
    },
    /** Shell feedback remains an external callback, separate from native layout ownership. */
    onPublish(callback: (value: EditorLayoutMetrics) => void): void {
      onPublish = callback;
    },
    /** Public disposal remains testable before and after queued READ. */
    destroy(): void {
      owner?.destroy();
    },
    /** Restore the complete global descriptor even when the actual owner reports a raw disconnect failure. */
    close(): void {
      const causes: unknown[] = [];
      for (const action of [
        () => owner?.destroy(),
        () => {
          if (descriptor) Object.defineProperty(globalThis, 'ResizeObserver', descriptor);
          else Reflect.deleteProperty(globalThis, 'ResizeObserver');
        },
      ])
        try {
          action();
        } catch (cause) {
          causes.push(cause);
        }
      if (causes.length === 1) throw causes[0];
      if (causes.length) throw new AggregateError(causes, 'native-layout-fixture-retirement');
    },
  };
}
