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
/** Share native widget subscriptions across independently bound fields on one exact node. */
import { isSerializedWidget } from '../../graph/WidgetValueSerialization.js';
import type { ComfyNode } from '../../types/graph.js';
import type { InputLoader } from '../domain/InputLoader.js';
interface Subscription {
  readonly loader: InputLoader;
  changed(intent: 'append' | 'replace' | null): void;
  report(error: unknown): void;
}
interface Owner {
  readonly subscriptions: Set<Subscription>;
  stop(): void;
}
const owners = new WeakMap<ComfyNode, Owner>();
/** Retain one native callback lease and one SimpleSyrup file-picker intent owner per node. */
export function observeInputLoaderWidgets(
  node: ComfyNode,
  loader: InputLoader,
  changed: Subscription['changed'],
  report: Subscription['report'],
): () => void {
  let owner = owners.get(node);
  if (!owner) {
    const subscriptions = new Set<Subscription>(),
      previous = node.onWidgetChanged,
      releases: (() => void)[] = [];
    let intent: 'append' | 'replace' | null = null;
    if (
      ['SimpleSyrup.LoadMaskBatch', 'SimpleSyrup.LoadImageList'].includes(
        node.type ?? node.class_type ?? '',
      )
    )
      for (const [name, action] of [
        ['simple_syrup_add_media', 'append'],
        ['simple_syrup_replace_media', 'replace'],
      ] as const) {
        const widget = node.widgets?.find((widget) => widget.name === name),
          callback = widget?.callback;
        if (!widget || !callback) continue;
        const wrapped = function (this: unknown, ...args: unknown[]): unknown {
          intent = action;
          return callback.apply(this, args);
        };
        widget.callback = wrapped;
        releases.push(() => {
          if (widget.callback === wrapped) widget.callback = callback;
        });
      }
    const listener = (...args: unknown[]): unknown => {
      const result = typeof previous === 'function' ? previous.apply(node, args) : undefined;
      if (
        typeof args[0] === 'string' &&
        node.widgets?.some((widget) => widget.name === args[0] && isSerializedWidget(widget))
      ) {
        const edit = intent;
        intent = null;
        for (const subscription of subscriptions) {
          try {
            subscription.changed(args[0] === subscription.loader.field ? edit : null);
          } catch (error: unknown) {
            subscription.report(error);
          }
        }
      }
      return result;
    };
    node.onWidgetChanged = listener;
    owner = {
      subscriptions,
      stop: () => {
        for (const release of releases) release();
        if (node.onWidgetChanged === listener) node.onWidgetChanged = previous;
      },
    };
    owners.set(node, owner);
  }
  const subscription = { loader, changed, report };
  owner.subscriptions.add(subscription);
  return () => {
    owner.subscriptions.delete(subscription);
    if (!owner.subscriptions.size && owners.get(node) === owner) {
      owner.stop();
      owners.delete(node);
    }
  };
}
