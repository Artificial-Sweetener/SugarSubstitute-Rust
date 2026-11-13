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
/** Local tooltip preparation, measured anchoring and spelling replacement delegate every publication to the sole native invocation owner. */
import type { EditorState } from '@codemirror/state';
import type { EditorView } from '@codemirror/view';
import {
  contextMenuTargetAt,
  prepareContextMenu,
  type MenuTarget,
} from './context-menu-preparation.js';
import {
  contextMenu,
  currentContextMenu,
  type ContextMenu,
  type PreparedContextMenu,
} from './context-menu-state.js';
import { nativeClipboard } from './native-clipboard.js';
import { spellingProvider } from './spelling-provider.js';
import {
  captureFocusHandoff,
  currentFocusHandoff,
  refocusCapturedHandoff,
} from './focus-handoff.js';
import type { FocusHandoff } from './focus-handoff-contracts.js';

/** Geometry owns no invocation, request lane, focus capability or edit registry. */
export interface LocalMenuPublication {
  readonly measurementKey: object;
  current(): boolean;
  prepare(state: EditorState, menu: PreparedContextMenu): boolean;
  publish(menu: ContextMenu): boolean;
  complete(): void;
  cancel(): void;
}
/** Platform gesture admission grants no invocation, selection or focus authority. */
export type LocalMenuKeyboardIntent = 'ignore' | 'dismiss' | 'open';

/** Embedded native inputs keep their keys; ordinary Escape and native menu gestures delegate to the invocation owner. */
export function localMenuKeyboardIntent(event: KeyboardEvent): LocalMenuKeyboardIntent {
  if (event.target instanceof Element && event.target.closest('input,textarea')) return 'ignore';
  if (event.key === 'Escape') return 'dismiss';
  return event.key === 'ContextMenu' ||
    (event.key === 'F10' && event.shiftKey && !event.ctrlKey && !event.altKey && !event.metaKey)
    ? 'open'
    : 'ignore';
}

/** Native menu controls consume their own composed pointer and captured-scroll paths, including shadow-host retargeting. */
export function localMenuContainsEventTarget(event: Event): boolean {
  return event
    .composedPath()
    .some(
      (target) =>
        target instanceof Element && !!target.closest('[data-sc-prompt-menu],.sc-context-menu'),
    );
}

/** Native chip or coordinate hit testing supplies logical-document UTF-16 without parsing source. */
export function localMenuPointerTarget(view: EditorView, event: MouseEvent): MenuTarget | null {
  if (
    view.composing ||
    view.compositionStarted ||
    !(event.target instanceof Element) ||
    !!event.target.closest('input,textarea') ||
    !view.contentDOM.contains(event.target)
  )
    return null;
  const chip = event.target.closest<HTMLElement>('.sp-lora-chip');
  const position = chip
    ? view.posAtDOM(chip, 0)
    : view.posAtCoords({ x: event.clientX, y: event.clientY });
  return position === null ? null : contextMenuTargetAt(view.state, position);
}

/** Capture the existing surface intent before invocation callbacks; refusals and failures retire only that original capability. */
export function openLocalPointerMenu(
  view: EditorView,
  target: MenuTarget,
  event: MouseEvent,
  request: (
    target: MenuTarget,
    point: { readonly x: number; readonly y: number },
    handoff: FocusHandoff,
  ) => boolean,
): boolean {
  const refocus = !view.hasFocus;
  const handoff = captureFocusHandoff(view, 'reserved-source');
  try {
    if (!request(target, { x: event.clientX, y: event.clientY }, handoff)) {
      handoff.cancel();
      return false;
    }
    event.preventDefault();
    if (refocus) {
      if (currentFocusHandoff(view, handoff)) void refocusCapturedHandoff(view, handoff);
      else handoff.cancel();
    }
    return true;
  } catch (cause) {
    try {
      handoff.cancel();
    } catch (cleanup) {
      throw new AggregateError([cause, cleanup], 'local-menu-focus-retirement-failed');
    }
    throw cause;
  }
}

/** Queue local preparation outside input and read coordinates only in the supported native measurement phase. */
export function requestLocalMenu(
  view: EditorView,
  target: MenuTarget,
  point: { readonly x: number; readonly y: number } | undefined,
  owner: LocalMenuPublication,
): void {
  queueMicrotask(() => {
    try {
      if (!owner.current()) {
        owner.cancel();
        return;
      }
      const state = view.state,
        currentTarget = contextMenuTargetAt(state, target.position),
        clipboard = view.plugin(nativeClipboard);
      if (!currentTarget) {
        owner.cancel();
        return;
      }
      const prepared = prepareContextMenu(state, currentTarget, 'local', {
        readable: clipboard?.readable ?? false,
        writable: clipboard?.writable ?? false,
      });
      if (
        !prepared ||
        !currentContextMenu(view.state, prepared) ||
        !owner.prepare(state, prepared)
      ) {
        owner.cancel();
        return;
      }
      view.requestMeasure({
        key: owner.measurementKey,
        read: (): ContextMenu | null => {
          try {
            if (!owner.current() || !currentContextMenu(view.state, prepared)) return null;
            const anchor = point
              ? { left: point.x, right: point.x, top: point.y, bottom: point.y }
              : view.coordsAtPos(target.position);
            return anchor ? { ...prepared, anchor } : null;
          } catch (cause) {
            failLocalMenu(owner, cause);
          }
        },
        write: (menu) =>
          queueMicrotask(() => {
            try {
              owner.complete();
              if (!menu || !owner.current() || !currentContextMenu(view.state, menu)) {
                owner.cancel();
                return;
              }
              if (owner.publish(menu)) refreshLocalSpelling(view, menu, owner);
              else owner.cancel();
            } catch (cause) {
              failLocalMenu(owner, cause);
            }
          }),
      });
    } catch (cause) {
      failLocalMenu(owner, cause);
    }
  });
}
/** Retirement cannot replace the original preparation/measurement/publication cause, including thrown null or undefined. */
function failLocalMenu(owner: LocalMenuPublication, cause: unknown): never {
  try {
    owner.cancel();
  } catch (cleanup) {
    throw new AggregateError([cause, cleanup], 'local-menu-publication-retirement-failed');
  }
  throw cause;
}
/** The existing finite spelling service may replace only a still-visible local invocation. */
function refreshLocalSpelling(
  view: EditorView,
  menu: ContextMenu,
  owner: LocalMenuPublication,
): void {
  if (menu.diagnostic?.kind !== 'spelling' || menu.diagnostic.suggestions) return;
  const provider = view.plugin(spellingProvider);
  if (!provider) return;
  const finding = menu.diagnostic.finding;
  void provider
    .suggest(finding)
    .then((outcome) => {
      if (outcome.status !== 'ready' && outcome.status !== 'unavailable') return;
      if (
        !owner.current() ||
        view.state.field(contextMenu) !== menu ||
        !currentContextMenu(view.state, menu)
      )
        return;
      const state = view.state,
        target = contextMenuTargetAt(state, menu.from),
        clipboard = view.plugin(nativeClipboard);
      if (target?.spellingDiagnostic?.finding !== finding) return;
      const prepared = prepareContextMenu(
        state,
        target,
        'local',
        {
          readable: clipboard?.readable ?? false,
          writable: clipboard?.writable ?? false,
        },
        outcome.status === 'ready' ? outcome.value : null,
        outcome.status === 'unavailable' ? 'unavailable' : 'pending',
      );
      if (!prepared || !currentContextMenu(view.state, prepared) || !owner.prepare(state, prepared))
        return;
      owner.publish({ ...prepared, anchor: menu.anchor });
    })
    .catch((cause: unknown) => provider.report(cause));
}
