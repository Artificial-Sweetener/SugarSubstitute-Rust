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
/** Own native editing permission configuration and its committed input-retirement lifetime. */
import { Compartment, EditorState, type Extension, type TransactionSpec } from '@codemirror/state';
import {
  cancelNativeComposition,
  previewNativeCompositionCancellation,
} from './composition-history.js';
import type { NativeSurface } from './native-surface.js';

/** Host admission is request-scoped; committed permission work belongs to the mounted lifecycle. */
export interface NativePermissionAuthority {
  /** Admit only the still-current complete host configuration before its transaction commits. */
  readonly current: () => boolean;
  /** Observe mounted lifetime independently of later unrelated host requests. */
  readonly mounted: () => boolean;
}

/** One mounted permission owner binds baseline configuration and superseding host intent. */
export class NativeEditingPermission {
  private readonly configuration = new Compartment();
  private value: boolean;
  private request = 0;

  /** Capture the initial host permission without acquiring native resources. */
  constructor(readOnly: boolean) {
    if (typeof readOnly !== 'boolean') throw new TypeError('Invalid permission');
    this.value = readOnly;
  }

  /** Matching current native state can still supersede a pending cancellation's permission intent. */
  matches(value: boolean): boolean {
    return this.value === value;
  }
  /** Privileged baseline adoption retires any older permission continuation while preserving final permission. */
  adoptBaseline(value: boolean): void {
    this.request++;
    this.value = value;
  }
  /** Bind the latest accepted permission to an ordinary state or baseline restoration. */
  extension(value = this.value): Extension {
    return this.configuration.of(EditorState.readOnly.of(value));
  }

  /** Final revocation validation delegates source projection to the same native cancellation/history owner. */
  preview(value: boolean, surface: Pick<NativeSurface, 'view'>): EditorState {
    return value ? previewNativeCompositionCancellation(surface.view) : surface.view.state;
  }
  /** Admit the whole host bundle before committing; permission, composition and mounted owners govern its queued retirement. */
  apply(
    value: boolean,
    surface: Pick<NativeSurface, 'view' | 'retireInput'>,
    authority: NativePermissionAuthority,
    publish: () => void,
    configure?: () => TransactionSpec,
  ): EditorState | undefined {
    const request = ++this.request;
    this.value = value;
    const view = surface.view,
      restart = value && cancelNativeComposition(view);
    if (!authority.current() || request !== this.request || surface.view !== view) return;
    const configured = configure?.() ?? {};
    const effects = configured.effects
      ? Array.isArray(configured.effects)
        ? configured.effects
        : [configured.effects]
      : [];
    const transaction = view.state.update({
      ...configured,
      effects: [this.configuration.reconfigure(EditorState.readOnly.of(value)), ...effects],
    });
    view.dispatch(transaction);
    if (restart) {
      /** A committed revocation survives unrelated host publication, while its actual owners still retire it. */
      const current = (): boolean =>
        authority.mounted() && request === this.request && surface.view === view && restart();
      if (current()) surface.retireInput(current, publish);
    }
    return transaction.state;
  }
}
