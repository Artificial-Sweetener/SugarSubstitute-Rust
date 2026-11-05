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
/** Bind an isolated overlay root to one revisioned surface port. */

import { createApp, h, shallowRef } from 'vue';
import { canvasMediaLabel } from '../CanvasMediaLabel.js';
import { ContextMenu } from '../../../menus/ContextMenu.js';
import { canvasContextActions } from './CanvasContextActions.js';
import { sameCanvasAttachment } from '../../domain/CanvasIdentity.js';
import type {
  CanvasCommand,
  CanvasCommandRequest,
  CanvasViewPort,
  CanvasViewSnapshot,
} from '../../domain/CanvasViewPort.js';
import { canvasNavigationSnapshot } from './CanvasNavigationSnapshot.js';
import { canvasMediaItem, type CanvasMediaTarget } from '../../domain/CanvasMedia.js';
import { CanvasOverlay } from './CanvasOverlay.js';
import { ensureCanvasOverlayStyles } from './CanvasOverlayStyles.js';
import type { CanvasRouteId, CanvasRoutePort } from '../../domain/CanvasRoute.js';

export interface MountedCanvasOverlay {
  /** Release this root's subscription and controls without releasing its surface. */
  dispose(): void;
}

/** Subscribe and release only presentation; surface resource lifetime stays with its owner. */
export function mountCanvasOverlay(
  element: HTMLElement,
  port: CanvasViewPort,
  reportFailure: (error: unknown, request: CanvasCommandRequest | null) => void,
  interactionSurface: HTMLElement = element,
  contextTarget: (target: EventTarget | null) => CanvasMediaTarget = () => 'primary',
  route: CanvasRoutePort | null = null,
): MountedCanvasOverlay {
  ensureCanvasOverlayStyles(element.ownerDocument);
  const initial = port.read();
  const state = shallowRef(initial);
  const routeState = shallowRef(route?.read() ?? null);
  const stopRoute = route?.subscribe(() => {
    if (active) routeState.value = route.read();
  });
  const announcement = shallowRef('');
  let active = true;
  let sequence = 0;
  /** Dispatch a capability-gated intent with its exact originating attachment and revision. */
  async function send(command: CanvasCommand): Promise<void> {
    if (!active || !port.capabilities.commands.includes(command.type)) return;
    const invocation = ++sequence;
    let request: CanvasCommandRequest | null = null;
    try {
      request = port.createRequest(command, state.value.revision);
      const outcome = await port.dispatch(request);
      if (!active || invocation !== sequence) return;
      if (outcome.status === 'error') reportFailure(outcome, request);
      announcement.value =
        outcome.status === 'error'
          ? outcome.message
          : outcome.status === 'stale'
            ? 'View updated; try again.'
            : '';
    } catch (error: unknown) {
      reportFailure(error, request);
      if (active && invocation === sequence) {
        announcement.value = error instanceof Error ? error.message : 'The canvas command failed.';
      }
    }
  }
  const menu = new ContextMenu(element.ownerDocument);
  let menuTarget: CanvasMediaTarget = 'primary';
  /** Open optional actions from this surface only, retaining its attachment-qualified dispatch. */
  function openMenu(event: MouseEvent | KeyboardEvent): void {
    if (
      event instanceof KeyboardEvent &&
      event.key !== 'ContextMenu' &&
      !(event.shiftKey && event.key === 'F10')
    )
      return;
    const target = event.target;
    if (target instanceof Element && target.closest('input,select,textarea')) return;
    event.preventDefault();
    event.stopPropagation();
    const bounds = interactionSurface.getBoundingClientRect();
    menuTarget = contextTarget(target);
    menu.open({
      label: 'Output actions',
      anchor: {
        kind: 'pointer',
        x: event instanceof MouseEvent ? event.clientX : bounds.left + 12,
        y: event instanceof MouseEvent ? event.clientY : bounds.bottom - 48,
      },
      trigger: target instanceof HTMLElement ? target : interactionSurface,
      entries: canvasContextActions(state.value, port.capabilities, send, menuTarget),
    });
  }
  interactionSurface.addEventListener('contextmenu', openMenu);
  interactionSurface.addEventListener('keydown', openMenu);
  /** Retire transient controls before their view or document is replaced. */
  function closeInteraction(): void {
    menu.close();
    interactionSurface.removeEventListener('contextmenu', openMenu);
    interactionSurface.removeEventListener('keydown', openMenu);
  }
  const unsubscribe = port.subscribe((snapshot) => {
    if (
      !active ||
      !sameCanvasAttachment(initial.address, snapshot.address) ||
      snapshot.revision <= state.value.revision
    )
      return;
    if (menu.isOpen()) {
      if (
        snapshot.selected?.id !== state.value.selected?.id ||
        snapshot.secondary?.id !== state.value.secondary?.id ||
        snapshot.preview !== state.value.preview ||
        snapshot.view.mode !== state.value.view.mode ||
        (typeof menuTarget === 'object' && !canvasMediaItem(snapshot, menuTarget))
      )
        menu.close();
      else if (
        canvasNavigationSnapshot(snapshot, state.value) !== state.value ||
        snapshot.mediaActions !== state.value.mediaActions
      )
        menu.updateEntries(canvasContextActions(snapshot, port.capabilities, send, menuTarget));
    }
    const spoken = canvasAnnouncement(state.value, snapshot);
    if (spoken !== null) announcement.value = spoken;
    state.value = snapshot;
  });
  const app = createApp({
    render: () =>
      h(CanvasOverlay, {
        snapshot: state.value,
        capabilities: port.capabilities,
        send,
        announcement: announcement.value,
        route: routeState.value,
        chooseRoute: (value: CanvasRouteId) => {
          if (active) route?.choose(value);
        },
      }),
  });
  try {
    app.mount(element);
  } catch (error: unknown) {
    active = false;
    closeInteraction();
    unsubscribe();
    stopRoute?.();
    reportFailure(error, null);
    throw error;
  }
  return {
    /** Make root teardown idempotent while invalidating pending command feedback. */
    dispose() {
      if (!active) return;
      active = false;
      closeInteraction();
      unsubscribe();
      stopRoute?.();
      app.unmount();
    },
  };
}

/** Announce meaningful navigation/completion/failure without narrating camera or preview frames. */
function canvasAnnouncement(
  previous: CanvasViewSnapshot,
  current: CanvasViewSnapshot,
): string | null {
  const source = current.sources.find((entry) => entry.id === current.view.selection?.sourceId);
  if (previous.view.selection?.sourceId !== current.view.selection?.sourceId)
    return source?.label ?? 'Output source is unavailable.';
  if (
    current.resource.state === 'error' &&
    (previous.resource.state !== 'error' || previous.resource.message !== current.resource.message)
  )
    return current.resource.message;
  if (
    !current.preview &&
    current.resource.state === 'ready' &&
    current.selected &&
    (previous.selected?.id !== current.selected.id ||
      previous.resource.state !== 'ready' ||
      previous.preview)
  )
    return `${source?.label ?? 'Output'}: ${canvasMediaLabel(current.selected.mediaKind)} ${current.selected.batchIndex + 1} ready.`;
  if (previous.connected && !current.connected) return 'Disconnected.';
  if (current.secondaryResource?.state === 'error' && previous.secondaryResource?.state !== 'error')
    return `B: ${current.secondaryResource.message}`;
  return null;
}
