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
/** Compose rail intent from the existing cart, canonical reorder and transient menu owners. */
import { ContextMenu } from '../../menus/ContextMenu.js';
import type { ComfyCubeRuntime } from '../../cube/ComfyCubeRuntime.js';
import type { CubeIdentitySource } from '../../cube/CubeIdentityPresentation.js';
import type { UnknownRecord } from '../../types/common.js';
import type { CubePickerHostIntegration } from '../../picker/CubePickerHostIntegration.js';
import type { AppearanceSource } from '../appearance/AppearanceState.js';
import type { CubeStackHostOptions } from './CubeStackHost.js';
import type { CubeWorkflowLibraryActions } from '../../workflow/CubeWorkflowLibraryActions.js';
import type { ModalService } from '../../dialogs/ModalService.js';

/** Expose only the current instance and canonical ordering capabilities consumed by the menu. */
export interface CubeStackCommandRuntime {
  readonly nodes: Pick<ComfyCubeRuntime['nodes'], 'get'>;
  readonly reorder: Pick<
    ComfyCubeRuntime['reorder'],
    'analysis' | 'available' | 'pending' | 'subscribe' | 'move' | 'reorder' | 'canMove' | 'refresh'
  >;
  readonly instanceActions: Pick<
    ComfyCubeRuntime['instanceActions'],
    'require' | 'rename' | 'duplicate' | 'remove' | 'toggleBypass' | 'toggleOutputPersistence'
  >;
}

/** Keep focused presentation commands separate from host registration and native graph mutation. */
export function createCubeStackCommands(options: {
  document: Document;
  appearance: AppearanceSource;
  frame?: CubeStackHostOptions['frame'];
  picker: Pick<CubePickerHostIntegration, 'openCart'>;
  library: Pick<CubeWorkflowLibraryActions, 'classification' | 'capture'>;
  dialogs: Pick<ModalService, 'promptText'>;
  getRuntime(): CubeStackCommandRuntime | null;
  resolveSource(metadata: UnknownRecord): CubeIdentitySource | null;
  reportError(error: unknown): void;
}): Omit<
  CubeStackHostOptions,
  'document' | 'app' | 'refreshSurface' | 'revealSearchResults' | 'reportError'
> {
  const menu = new ContextMenu(options.document);
  /** Supply existing icon-font glyphs without rendering external markup. */
  const icon =
    (name: string) =>
    (document: Document): HTMLElement => {
      const element = document.createElement('i');
      element.className = `pi ${name}`;
      return element;
    };
  /** Report rejected instance operations through the established SugarCubes feedback path. */
  const run = (work: () => unknown | Promise<unknown>): void => {
    void Promise.resolve().then(work).catch(options.reportError);
  };
  /** Resolve the current graph-bound owner at invocation instead of retaining retired menus. */
  const actions = () => {
    const runtime = options.getRuntime();
    if (!runtime) throw new Error('The Cube workflow is unavailable.');
    return runtime.instanceActions;
  };
  return {
    appearance: options.appearance,
    ...(options.frame ? { frame: options.frame } : {}),
    closeTransient: () => menu.close(),
    getReorder: () => options.getRuntime()?.reorder ?? null,
    resolveSource: options.resolveSource,
    openCart: () => options.picker.openCart(),
    remove: (id) => {
      const runtime = options.getRuntime();
      const node = runtime?.nodes.get(id);
      run(() => {
        if (!runtime || options.getRuntime() !== runtime || runtime.nodes.get(id) !== node)
          throw new Error('The Cube workflow changed before removal.');
        runtime.instanceActions.remove(id);
      });
    },
    contextMenu: (id, event) => {
      const runtime = options.getRuntime();
      if (!runtime) return;
      const node = runtime.nodes.get(id);
      if (!node) return;
      const service = runtime.reorder;
      /** Reject a menu action when its original workflow or Cube has been replaced. */
      const current = () => {
        if (options.getRuntime() !== runtime || runtime.nodes.get(id) !== node)
          throw new Error('The Cube workflow changed while its menu was open.');
        return actions();
      };
      const disabled = service.pending;
      const captureAvailable =
        options.library.classification(id)?.permittedOperations.has('capture') === true;
      menu.open({
        label: 'Cube stack',
        anchor: { kind: 'pointer', x: event.clientX, y: event.clientY },
        trigger: event.currentTarget instanceof HTMLElement ? event.currentTarget : null,
        attributes: {
          'data-sc-cube-stack-menu': '',
          'data-appearance': options.appearance.appearance.fluent ? 'fluent' : 'comfy',
        },
        entries: [
          {
            label:
              node.properties.sugarcubes_save_outputs === false
                ? 'Save outputs'
                : "Don't save outputs",
            disabled,
            select: () => run(() => current().toggleOutputPersistence(id)),
          },
          {
            label: 'Rename',
            icon: icon('pi-pencil'),
            disabled,
            select: () =>
              run(async () => {
                current().require(id);
                const name = await options.dialogs.promptText({
                  title: 'Rename Cube',
                  label: 'Name',
                  initialValue: node.title ?? node.subgraph.name,
                  confirmLabel: 'Rename',
                  normalizeValue: (value) => value.trim(),
                });
                if (name !== null) current().rename(id, name);
              }),
          },
          {
            label: 'Duplicate',
            icon: icon('pi-copy'),
            disabled,
            select: () => run(() => current().duplicate(id)),
          },
          ...(captureAvailable
            ? [
                {
                  label: 'Capture Cube',
                  icon: icon('pi-save'),
                  disabled,
                  select: () =>
                    run(() => {
                      current().require(id);
                      return options.library.capture(id);
                    }),
                },
              ]
            : []),
          {
            icon: icon('pi-pause'),
            label: node.mode === 4 ? 'Remove bypass' : 'Bypass',
            disabled,
            select: () => run(() => current().toggleBypass(id)),
          },
          {
            label: 'Remove',
            icon: icon('pi-trash'),
            disabled,
            select: () => run(() => current().remove(id)),
          },
          { label: '', separator: true },
          {
            label: 'Move up',
            disabled: !service?.canMove(id, -1),
            select: () => {
              run(() => {
                current().require(id);
                return service.move(id, -1);
              });
            },
          },
          {
            label: 'Move down',
            disabled: !service?.canMove(id, 1),
            select: () => {
              run(() => {
                current().require(id);
                return service.move(id, 1);
              });
            },
          },
          { label: 'Edit stack', select: () => options.picker.openCart() },
        ],
      });
    },
  };
}
