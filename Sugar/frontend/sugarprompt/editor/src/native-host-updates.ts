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
/** One native adoption owner prepares host bundles and arbitrates reentrant publication. */
import { requireValue } from '../../core/src/required.js';
import type { EditorState } from '@codemirror/state';
import type { NativeSurface } from './native-surface.js';
import type { NativeConfiguration } from './native-configuration.js';
import type { NativeEditingPermission } from './native-editing-permission.js';
import type { HostUpdate, HostUpdateOutcome } from './host-update.js';
import { prepareHostDocument, type BaselinePreparation } from './host-document-preparation.js';
import { nativeCompositionStarted } from './input-settlement.js';
import { featureProfile } from './features.js';
import type { FeatureProfile } from '../../services/src/index.js';
import { NativeDocumentCheckpoints } from './native-document-checkpoints.js';
import type { DocumentCaptureOutcome } from './document-checkpoint.js';
/** The mounted editor retains native view, source revision and finite disposal authority. */
export interface NativeHostUpdateOwner {
  readonly surface: Pick<
    NativeSurface,
    'view' | 'setState' | 'retireHandoff' | 'retireInput' | 'handoff'
  >;
  readonly configuration: NativeConfiguration;
  readonly permission: NativeEditingPermission;
  readonly createBaseline: BaselinePreparation;
  readonly alive: () => boolean;
  readonly revision: () => number;
  readonly adoptBaseline: () => void;
  readonly publish: () => void;
  /** The native commit acknowledges only its still-effective prepared profile before observable publication. */
  readonly accepted?: (profile: FeatureProfile) => void;
}
/** Reentrant accepted host requests supersede older source/configuration/focus adoption. */
export class NativeHostUpdates {
  private readonly owner: NativeHostUpdateOwner;
  private request = 0;
  private depth = 0;
  private readonly documents: NativeDocumentCheckpoints;
  /** Capture collaborators without retaining another source, selection or history buffer. */
  constructor(owner: NativeHostUpdateOwner) {
    this.owner = owner;
    this.documents = new NativeDocumentCheckpoints({
      view: () => owner.surface.view,
      alive: owner.alive,
    });
  }
  /** Capture accepted native input without exposing state configuration or derived resource fields. */
  async captureDocument(signal?: AbortSignal): Promise<DocumentCaptureOutcome> {
    const request = this.request + 1;
    this.supersede();
    if (this.request !== request)
      return Promise.resolve({
        status: 'rejected',
        reason: this.owner.alive() ? 'superseded' : 'disposed',
      });
    return this.documents.capture(signal);
  }
  /** Revoke retained document fields before the controller releases its native surface. */
  dispose(): void {
    this.request++;
    this.documents.dispose();
  }
  /** Only complete native bundles publish; nested operations share the finite publication boundary. */
  get publishing(): boolean {
    return this.depth > 0;
  }
  /** Explicit public selection/source intent supersedes an in-flight host publication. */
  supersede(): void {
    this.request++;
    this.documents.supersede();
  }
  /** Validate and prepare before admission changes any native authority. */
  apply(update: HostUpdate): HostUpdateOutcome {
    const owner = this.owner;
    if (!owner.alive()) {
      if (update?.mode === 'restore-document') return { status: 'rejected', reason: 'disposed' };
      throw new Error('Editor disposed');
    }
    const initialRequest = this.request;
    if (
      !update ||
      typeof update !== 'object' ||
      Array.isArray(update) ||
      !['preserve', 'exact-edit', 'baseline', 'restore-document'].includes(update.mode)
    )
      throw new TypeError('Invalid host update');
    const keys =
      update.mode === 'preserve'
        ? ['mode', 'configuration', 'selection', 'focus']
        : update.mode === 'restore-document'
          ? ['mode', 'checkpoint', 'configuration', 'selection', 'focus']
          : ['mode', 'source', 'configuration', 'selection', 'focus'];
    if (Object.keys(update).some((key) => !keys.includes(key)))
      throw new TypeError('Invalid host update field');
    if (
      ('configuration' in update &&
        (!update.configuration ||
          typeof update.configuration !== 'object' ||
          Array.isArray(update.configuration))) ||
      ('selection' in update &&
        (!update.selection ||
          typeof update.selection !== 'object' ||
          Array.isArray(update.selection)))
    )
      throw new TypeError('Invalid host update value');
    if ('focus' in update && update.focus !== 'preserve' && update.focus !== 'editor')
      throw new TypeError('Invalid focus intent');
    const view = owner.surface.view,
      state = view.state;
    const checkpoint = update.mode === 'restore-document' ? update.checkpoint : undefined;
    const document = checkpoint ? this.documents.inspect(checkpoint) : undefined;
    if (update.mode === 'restore-document') {
      if (!checkpoint) return { status: 'rejected', reason: 'foreign-checkpoint' };
      if (typeof document === 'string') return { status: 'rejected', reason: document };
      if (nativeCompositionStarted(view)) return { status: 'rejected', reason: 'composing' };
    }
    const validationState =
      update.mode === 'preserve' &&
      update.configuration?.readOnly === true &&
      nativeCompositionStarted(view)
        ? owner.permission.preview(true, owner.surface)
        : state;
    const preparedDocument = prepareHostDocument(
      validationState,
      update,
      owner.configuration,
      owner.revision(),
      owner.createBaseline,
      typeof document === 'string' ? undefined : document,
    );
    if (update.mode === 'restore-document') {
      const validated = this.documents.inspect(requireValue(checkpoint, 'native-host-updates'));
      if (typeof validated === 'string') return { status: 'rejected', reason: validated };
      if (
        this.request !== initialRequest ||
        !owner.alive() ||
        owner.surface.view !== view ||
        view.state !== state
      )
        return { status: 'rejected', reason: 'superseded' };
    }
    const {
      configuration: prepared,
      transaction,
      baseline,
      readOnly,
      sourceChanged,
    } = preparedDocument;
    if (update.mode !== 'preserve' && nativeCompositionStarted(view))
      return { status: 'rejected', reason: 'composing' };
    if (update.mode === 'exact-edit' && (state.readOnly || readOnly))
      return { status: 'rejected', reason: 'read-only' };
    const permissionChanged = readOnly !== state.readOnly || !owner.permission.matches(readOnly);
    if (
      !baseline &&
      !sourceChanged &&
      !transaction.selection &&
      !transaction.effects.length &&
      !permissionChanged &&
      update.focus !== 'editor'
    )
      return { status: 'unchanged' };
    const request = ++this.request;
    this.documents.supersede();
    const current = (): boolean =>
      owner.alive() && request === this.request && owner.surface.view === view;
    if (
      update.mode !== 'preserve' ||
      update.selection ||
      permissionChanged ||
      update.focus === 'editor'
    )
      owner.surface.retireHandoff(
        update.mode === 'preserve' && update.focus !== 'editor'
          ? 'preserve-input'
          : 'replace-input',
      );
    const handoff = update.focus === 'editor' ? owner.surface.handoff() : undefined;
    if (update.mode === 'restore-document') {
      const validated = this.documents.inspect(requireValue(checkpoint, 'native-host-updates'));
      if (typeof validated === 'string') {
        handoff?.cancel();
        return { status: 'rejected', reason: validated };
      }
      if (!current() || view.state !== state) {
        handoff?.cancel();
        return { status: 'rejected', reason: 'superseded' };
      }
      if (nativeCompositionStarted(view)) {
        handoff?.cancel();
        return { status: 'rejected', reason: 'composing' };
      }
      if (!current() || view.state !== state) {
        handoff?.cancel();
        return { status: 'rejected', reason: 'superseded' };
      }
    }
    let publishing = !permissionChanged || Boolean(baseline);
    if (publishing) this.depth++;
    let acceptedState: EditorState = transaction.state;
    try {
      if (baseline) {
        owner.configuration.accept(prepared);
        owner.permission.adoptBaseline(readOnly);
        owner.adoptBaseline();
        acceptedState = owner.surface.setState(baseline, handoff);
      } else if (permissionChanged) {
        const appliedState = owner.permission.apply(
          readOnly,
          owner.surface,
          { current, mounted: owner.alive },
          owner.publish,
          () => {
            this.depth++;
            publishing = true;
            owner.configuration.accept(prepared);
            return preparedDocument.specification;
          },
        );
        if (appliedState) acceptedState = appliedState;
      } else {
        owner.configuration.accept(prepared);
        view.dispatch(transaction);
      }
    } finally {
      if (publishing) this.depth--;
    }
    if (
      owner.alive() &&
      owner.configuration.values === prepared.values &&
      (!prepared.values.featureProfile ||
        owner.surface.view.state.facet(featureProfile) === prepared.values.featureProfile)
    )
      owner.accepted?.(owner.surface.view.state.facet(featureProfile));
    if (!current()) return { status: 'rejected', reason: 'superseded' };
    const accepted = view.state;
    if (accepted !== acceptedState) return { status: 'rejected', reason: 'superseded' };
    owner.publish();
    if (!current() || view.state !== accepted) return { status: 'rejected', reason: 'superseded' };
    const focusReady = handoff?.restore();
    if (!current()) return { status: 'rejected', reason: 'superseded' };
    return { status: 'applied', ...(focusReady ? { focusReady } : {}) };
  }
}
