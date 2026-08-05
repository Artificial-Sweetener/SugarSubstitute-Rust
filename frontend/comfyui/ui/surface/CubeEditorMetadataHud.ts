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
/** Present a pinned, native-styled Cube metadata card inside Comfy's editor viewport. */

import {
  isDraftCubeNode,
  requireCubeIdentity,
  type CubeNode,
} from '../cube/node/ComfyCubeNodeFactory.js';
import { isCubeAwaitingFirstSave } from '../cube/CubeIdentityPresentation.js';
import { defaultSupportedModelsForTarget } from '../core/ModelTargets.js';
import {
  ComfySettingsAutocompleteControl,
  ComfySettingsSingleSelectControl,
  InstalledComfySettingsSelectRenderer,
  type ComfySettingsSelectRenderer,
} from '../controls/ComfySettingsSelect.js';
import {
  CUSTOM_TARGET_MODEL_VALUE,
  isCustomTargetModel,
  supportedModelSuggestions,
  targetModelSelectOptions,
} from '../controls/CubeModelSelection.js';
import { createComfyPrimeIconElement } from './ComfyPrimeIcons.js';
import {
  CubeEditorMetadataDraftStore,
  type CubeEditorMetadataValues,
} from './CubeEditorMetadataDraftStore.js';
import { CubeEditorHudPositioner } from './CubeEditorHudPositioner.js';
import type { CubeEditorWorkspaceChromeAdapter } from './CubeEditorWorkspaceChromeAdapter.js';
import { createCubeUnsavedIndicator } from './CubeUnsavedIndicator.js';

const FOCUS_KEY_ATTRIBUTE = 'data-sugarcubes-focus-key';

export type { CubeEditorMetadataValues } from './CubeEditorMetadataDraftStore.js';

/** Distinguish a completed metadata save from a dismissed first-save dialog. */
export type CubeEditorMetadataSaveOutcome = 'saved' | 'cancelled';

export interface CubeEditorMetadataHudActions {
  canEdit(node: CubeNode): Promise<boolean>;
  save(node: CubeNode, values: CubeEditorMetadataValues): Promise<CubeEditorMetadataSaveOutcome>;
  modelSuggestions?(): readonly string[];
}

export interface CubeEditorMetadataHudOptions {
  settingsSelectRenderer?: ComfySettingsSelectRenderer;
  workspaceChrome?: CubeEditorWorkspaceChromeAdapter;
}

/** Own the non-graph, viewport-pinned Cube metadata surface for one active Cube editor. */
export class CubeEditorMetadataHud {
  readonly #document: Document;
  readonly #actions: CubeEditorMetadataHudActions;
  readonly #settingsSelectRenderer: ComfySettingsSelectRenderer;
  readonly #workspaceChrome: CubeEditorWorkspaceChromeAdapter | null;
  readonly #drafts = new CubeEditorMetadataDraftStore();
  readonly #settingsSelectControls = new Set<
    ComfySettingsAutocompleteControl | ComfySettingsSingleSelectControl
  >();
  #root: HTMLElement | null = null;
  #activeNode: CubeNode | null = null;
  #editable = false;
  #collapsed = false;
  #values: CubeEditorMetadataValues | null = null;
  #dirty = false;
  #saveError = '';
  #requestId = 0;
  #supportedModelsTouched = false;
  #customTargetModelSelected = false;
  #positioner: CubeEditorHudPositioner | null = null;

  /** Bind the HUD to its document and authoritative metadata actions. */
  constructor(
    documentRef: Document,
    actions: CubeEditorMetadataHudActions,
    options: CubeEditorMetadataHudOptions = {},
  ) {
    this.#document = documentRef;
    this.#actions = actions;
    this.#settingsSelectRenderer =
      options.settingsSelectRenderer ?? new InstalledComfySettingsSelectRenderer(documentRef);
    this.#workspaceChrome = options.workspaceChrome ?? null;
  }

  /** Clear the viewport card when the user returns to the root workflow. */
  hide(): void {
    this.#workspaceChrome?.leave();
    this.#retainActiveDraft();
    this.#activeNode = null;
    this.#values = null;
    this.#dirty = false;
    this.#saveError = '';
    this.#supportedModelsTouched = false;
    this.#customTargetModelSelected = false;
    this.#positioner?.dispose();
    this.#positioner = null;
    this.#releaseMountedControls();
    this.#root?.remove();
    this.#root = null;
  }

  /** Reconcile the card with the Cube currently active in Comfy's native editor. */
  show(node: CubeNode): void {
    if (this.#activeNode === node) return;
    this.#workspaceChrome?.enter();
    this.#retainActiveDraft();
    const retained = this.#drafts.get(node);
    this.#activeNode = node;
    this.#collapsed = !isDraftCubeNode(node);
    this.#values = retained?.values ?? readValues(node);
    this.#dirty = retained?.dirty ?? false;
    this.#saveError = retained?.saveError ?? '';
    this.#supportedModelsTouched = retained?.supportedModelsTouched ?? false;
    this.#customTargetModelSelected =
      retained?.customTargetModelSelected ??
      isCustomTargetModel(this.#values.targetModel, this.#actions.modelSuggestions?.());
    this.#editable = false;
    const requestId = ++this.#requestId;
    this.#render();
    void this.#actions.canEdit(node).then(
      (editable) => {
        if (requestId !== this.#requestId || this.#activeNode !== node) return;
        this.#editable = editable;
        this.#render();
      },
      () => {
        if (requestId !== this.#requestId || this.#activeNode !== node) return;
        this.#editable = false;
        this.#render();
      },
    );
  }

  /** Release the pinned viewport element. */
  dispose(): void {
    this.hide();
    this.#drafts.clear();
  }

  /** Render a title-only rolled-up card or the full metadata editor. */
  #render(): void {
    const node = this.#activeNode;
    const values = this.#values;
    if (!node || !values) {
      this.hide();
      return;
    }
    const root = this.#root ?? this.#createRoot();
    const focusedControl = captureFocusedControl(this.#document, root);
    this.#releaseMountedControls();
    root.classList.toggle('is-collapsed', this.#collapsed);
    root.classList.toggle('is-readonly', !this.#editable);
    const titlebar = this.#document.createElement('header');
    titlebar.className = 'sugarcubes-cube-editor-metadata__titlebar';
    const title = this.#document.createElement('strong');
    title.textContent = deriveDefaultAlias(values) || node.title || 'Untitled Cube';
    titlebar.append(title);
    if (isCubeAwaitingFirstSave(requireCubeIdentity(node))) {
      titlebar.append(createCubeUnsavedIndicator(this.#document));
    }
    if (this.#editable) titlebar.append(this.#createSaveButton());
    titlebar.append(this.#createCollapseButton());
    root.replaceChildren(titlebar);
    if (!this.#collapsed) root.append(this.#createBody(node, values));
    restoreFocusedControl(root, focusedControl);
  }

  /** Create the one viewport-fixed card host outside Comfy's graph model. */
  #createRoot(): HTMLElement {
    const root = this.#document.createElement('aside');
    root.className = 'sugarcubes-cube-editor-metadata';
    root.setAttribute('aria-label', 'Cube metadata');
    // Comfy's workspace chrome establishes its own stacking contexts inside #vue-app.
    // A viewport HUD must sit above that chrome so its controls cannot click through.
    this.#document.body.append(root);
    this.#root = root;
    this.#positioner = new CubeEditorHudPositioner(this.#document, root);
    return root;
  }

  /** Create the expanded metadata content for an editable or read-only Cube. */
  #createBody(node: CubeNode, values: CubeEditorMetadataValues): HTMLElement {
    const body = this.#document.createElement('div');
    body.className = 'sugarcubes-cube-editor-metadata__body';
    if (this.#editable) {
      body.append(
        this.#createField('Name', this.#createTextInput('defaultAlias', values.defaultAlias)),
        this.#createField('Target model', this.#createTargetModelEditor(values.targetModel)),
        this.#createField(
          'Supported models',
          this.#createSupportedModelsEditor(values.supportedModels),
        ),
        this.#createField('Description', this.#createDescriptionInput(values.description)),
      );
      if (isDraftCubeNode(node)) {
        body.append(
          this.#createField('Save to', this.#createDestinationSelect(values.destination)),
        );
      }
      if (this.#saveError) body.append(this.#createSaveError(this.#saveError));
      return body;
    }
    body.append(
      this.#createReadOnlyField('Target model', values.targetModel || 'Not specified'),
      this.#createReadOnlyField(
        'Supported models',
        values.supportedModels.length ? values.supportedModels.join(', ') : 'Not specified',
      ),
      this.#createReadOnlyField('Description', values.description || 'No description'),
      this.#createReadOnlyField('Access', 'Read only'),
    );
    return body;
  }

  /** Present a save failure beside the controls that can resolve it. */
  #createSaveError(message: string): HTMLElement {
    const error = this.#document.createElement('p');
    error.className = 'sugarcubes-cube-editor-metadata__error';
    error.setAttribute('role', 'alert');
    error.textContent = message;
    return error;
  }

  /** Create a labeled editable field without inserting dynamic markup. */
  #createField(labelText: string, control: HTMLElement): HTMLElement {
    const field = this.#document.createElement('label');
    field.className = 'sugarcubes-cube-editor-metadata__field';
    const label = this.#document.createElement('span');
    label.textContent = labelText;
    field.append(label, control);
    return field;
  }

  /** Create a compact read-only metadata row. */
  #createReadOnlyField(labelText: string, value: string): HTMLElement {
    const field = this.#document.createElement('div');
    field.className = 'sugarcubes-cube-editor-metadata__field';
    const label = this.#document.createElement('span');
    label.textContent = labelText;
    const content = this.#document.createElement('strong');
    content.textContent = value;
    field.append(label, content);
    return field;
  }

  /** Create one text control whose value remains available after the card rolls up. */
  #createTextInput(
    name: keyof Pick<CubeEditorMetadataValues, 'defaultAlias'>,
    value: string,
  ): HTMLInputElement {
    const input = this.#document.createElement('input');
    input.className = 'p-inputtext p-component';
    input.setAttribute(FOCUS_KEY_ATTRIBUTE, name);
    input.value = value;
    input.addEventListener('input', () => this.#updateValue(name, input.value));
    return input;
  }

  /** Reuse Comfy's editable Settings AutoComplete for model support. */
  #createSupportedModelsEditor(models: readonly string[]): HTMLElement {
    const control = new ComfySettingsAutocompleteControl(
      this.#document,
      this.#settingsSelectRenderer,
      {
        ariaLabel: 'Supported models',
        options: supportedModelSuggestions(this.#actions.modelSuggestions?.() ?? [], models),
        placeholder: 'Type a model family and press Enter',
        values: models,
        onChange: (values) => {
          this.#supportedModelsTouched = true;
          this.#updateValues({ supportedModels: [...values] }, false);
        },
      },
    );
    control.element.classList.add('sugarcubes-cube-editor-metadata__model-support');
    this.#settingsSelectControls.add(control);
    return control.element;
  }

  /** Reuse the established authoring target-model selector and automatic defaults. */
  #createTargetModelEditor(value: string): HTMLElement {
    const container = this.#document.createElement('div');
    container.className = 'sugarcubes-cube-editor-metadata__target-model';
    const combo = this.#document.createElement('div');
    combo.className = 'sugarcubes-cube-editor-metadata__target-model-combo';
    const custom = this.#document.createElement('input');
    custom.className = 'p-inputtext p-component';
    custom.setAttribute(FOCUS_KEY_ATTRIBUTE, 'customTargetModel');
    custom.placeholder = 'Enter target model';
    const suggestions = this.#actions.modelSuggestions?.() ?? [];
    const isCustom = this.#customTargetModelSelected || isCustomTargetModel(value, suggestions);
    custom.value = isCustom ? value : '';
    custom.hidden = !isCustom;
    custom.disabled = !isCustom;
    const control = new ComfySettingsSingleSelectControl(
      this.#document,
      this.#settingsSelectRenderer,
      {
        ariaLabel: 'Target model',
        options: targetModelSelectOptions(suggestions),
        value: isCustom ? CUSTOM_TARGET_MODEL_VALUE : value,
        onChange: (selected) => {
          const customSelected = selected === CUSTOM_TARGET_MODEL_VALUE;
          this.#customTargetModelSelected = customSelected;
          this.#updateTargetModel(customSelected ? custom.value : selected);
          if (customSelected) {
            queueMicrotask(() => {
              this.#root
                ?.querySelector<HTMLInputElement>(`[${FOCUS_KEY_ATTRIBUTE}="customTargetModel"]`)
                ?.focus();
            });
          }
        },
      },
    );
    combo.replaceChildren(control.element);
    this.#settingsSelectControls.add(control);
    custom.addEventListener('input', () => this.#updateTargetModel(custom.value));
    container.append(combo, custom);
    return container;
  }

  /** Create one description editor using Comfy's existing text-control classes. */
  #createDescriptionInput(value: string): HTMLTextAreaElement {
    const input = this.#document.createElement('textarea');
    input.className = 'p-inputtextarea p-inputtext p-component';
    input.setAttribute(FOCUS_KEY_ATTRIBUTE, 'description');
    input.rows = 3;
    input.value = value;
    input.addEventListener('input', () => this.#updateValue('description', input.value));
    return input;
  }

  /** Apply established model defaults until an author customizes model support. */
  #updateTargetModel(targetModel: string): void {
    if (!this.#values) return;
    this.#updateValues({
      targetModel,
      supportedModels: this.#supportedModelsTouched
        ? this.#values.supportedModels
        : defaultSupportedModelsForTarget(targetModel),
    });
  }

  /** Let first-save authors choose a local Cube or a writable author pack. */
  #createDestinationSelect(value: CubeEditorMetadataValues['destination']): HTMLElement {
    const control = new ComfySettingsSingleSelectControl(
      this.#document,
      this.#settingsSelectRenderer,
      {
        ariaLabel: 'Save to',
        options: [
          { label: 'Personal cubes', value: 'local' },
          { label: 'Author pack', value: 'pack' },
        ],
        value,
        onChange: (selected) =>
          this.#updateValue('destination', selected === 'pack' ? 'pack' : 'local'),
      },
    );
    this.#settingsSelectControls.add(control);
    return control.element;
  }

  /** Persist one input value in the HUD state while retaining the rolled-up save action. */
  #updateValue<K extends keyof CubeEditorMetadataValues>(
    key: K,
    value: CubeEditorMetadataValues[K],
  ): void {
    if (key === 'supportedModels') this.#supportedModelsTouched = true;
    this.#updateValues({ [key]: value } as Pick<CubeEditorMetadataValues, K>, false);
  }

  /** Persist one form update while preserving the rolled-up save action. */
  #updateValues(values: Partial<CubeEditorMetadataValues>, render = true): void {
    if (!this.#values) return;
    this.#values = { ...this.#values, ...values };
    this.#dirty = true;
    this.#saveError = '';
    this.#retainActiveDraft();
    if (render) {
      this.#render();
      return;
    }
    this.#refreshEditedState();
  }

  /** Refresh stateful chrome without replacing the field the user is actively editing. */
  #refreshEditedState(): void {
    const root = this.#root;
    const values = this.#values;
    if (!root || !values) return;
    const title = root.querySelector<HTMLElement>(
      '.sugarcubes-cube-editor-metadata__titlebar strong',
    );
    if (title) title.textContent = deriveDefaultAlias(values);
    const save = root.querySelector<HTMLButtonElement>('.sugarcubes-cube-editor-metadata__save');
    if (save) save.disabled = false;
    root.querySelector('.sugarcubes-cube-editor-metadata__error')?.remove();
  }

  /** Create the save control that remains visible in both expanded and rolled-up states. */
  #createSaveButton(): HTMLButtonElement {
    const button = this.#document.createElement('button');
    button.type = 'button';
    button.className = 'p-button p-component sugarcubes-cube-editor-metadata__save';
    button.setAttribute('aria-label', 'Save');
    button.title = 'Save Cube';
    button.append(createComfyPrimeIconElement(this.#document, 'save'));
    button.disabled = !this.#dirty && !isDraftCubeNode(this.#activeNode);
    button.addEventListener('click', () => {
      const node = this.#activeNode;
      const values = this.#values;
      if (!node || !values) return;
      button.disabled = true;
      void this.#actions.save(node, normalizeValues(values)).then(
        (outcome) => {
          if (outcome === 'cancelled') {
            if (node !== this.#activeNode) return;
            this.#render();
            return;
          }
          this.#drafts.delete(node);
          if (node !== this.#activeNode) return;
          this.#values = readValues(node);
          this.#dirty = false;
          this.#saveError = '';
          this.#render();
        },
        (error: unknown) => {
          if (node !== this.#activeNode) return;
          this.#saveError = readErrorMessage(error);
          this.#retainActiveDraft();
          this.#render();
        },
      );
    });
    return button;
  }

  /** Create the card's explicit roll-up affordance. */
  #createCollapseButton(): HTMLButtonElement {
    const button = this.#document.createElement('button');
    button.type = 'button';
    button.className = 'p-button p-component sugarcubes-cube-editor-metadata__collapse';
    const label = this.#collapsed ? 'Expand' : 'Minimize';
    button.setAttribute('aria-label', label);
    button.title = label;
    button.append(
      createComfyPrimeIconElement(this.#document, this.#collapsed ? 'chevron-down' : 'chevron-up'),
    );
    button.setAttribute('aria-expanded', String(!this.#collapsed));
    button.addEventListener('click', () => {
      this.#collapsed = !this.#collapsed;
      this.#render();
    });
    return button;
  }

  /** Release every mounted control before replacing or removing its DOM host. */
  #releaseMountedControls(): void {
    for (const control of this.#settingsSelectControls) control.dispose();
    this.#settingsSelectControls.clear();
  }

  /** Retain dirty authoring state before navigation removes the viewport card. */
  #retainActiveDraft(): void {
    if (!this.#activeNode || !this.#values || !this.#dirty) return;
    this.#drafts.set(this.#activeNode, {
      customTargetModelSelected: this.#customTargetModelSelected,
      dirty: this.#dirty,
      saveError: this.#saveError,
      supportedModelsTouched: this.#supportedModelsTouched,
      values: this.#values,
    });
  }
}

interface FocusedControl {
  key: string;
  selectionEnd: number | null;
  selectionStart: number | null;
}

/** Capture one typed field before a state refresh replaces the metadata card contents. */
function captureFocusedControl(documentRef: Document, root: HTMLElement): FocusedControl | null {
  const active = documentRef.activeElement;
  if (
    !root.contains(active) ||
    !(active instanceof HTMLInputElement || active instanceof HTMLTextAreaElement)
  ) {
    return null;
  }
  const key = active.getAttribute(FOCUS_KEY_ATTRIBUTE);
  return key
    ? {
        key,
        selectionEnd: active.selectionEnd,
        selectionStart: active.selectionStart,
      }
    : null;
}

/** Restore typing focus after the card refreshes its header and dependent field values. */
function restoreFocusedControl(root: HTMLElement, focused: FocusedControl | null): void {
  if (!focused) return;
  const control = root.querySelector<HTMLInputElement | HTMLTextAreaElement>(
    `[${FOCUS_KEY_ATTRIBUTE}="${focused.key}"]`,
  );
  if (!control || control.disabled || control.hidden) return;
  control.focus({ preventScroll: true });
  if (focused.selectionStart === null || focused.selectionEnd === null) return;
  const maximum = control.value.length;
  control.setSelectionRange(
    Math.min(focused.selectionStart, maximum),
    Math.min(focused.selectionEnd, maximum),
  );
}

/** Read the graph-owned metadata into the independent viewport editing state. */
function readValues(node: CubeNode): CubeEditorMetadataValues {
  const metadata = requireCubeIdentity(node);
  const targetModel = readString(metadata.target_model) || (isDraftCubeNode(node) ? 'SDXL' : '');
  const supportedModels = readStringArray(metadata.supported_models);
  const defaultAlias = readString(metadata.default_alias) || node.title || 'Untitled Cube';
  return {
    defaultAlias: deriveAuthoringName(defaultAlias),
    targetModel,
    supportedModels:
      supportedModels.length || !isDraftCubeNode(node) ? supportedModels : [targetModel],
    description: readString(metadata.description),
    destination: readDestination(metadata.cube_id),
  };
}

/** Retain the original save flow's basename-only editable Cube name. */
function deriveAuthoringName(defaultAlias: string): string {
  return defaultAlias.split('/').pop()?.trim() || defaultAlias.trim();
}

/** Derive the persisted-style alias from the target model and authoring name. */
function deriveDefaultAlias(values: CubeEditorMetadataValues): string {
  const name = values.defaultAlias.trim();
  const targetModel = values.targetModel.trim();
  if (!name) return '';
  return targetModel ? `${targetModel}/${name}` : name;
}

/** Normalize user-entered fields once, immediately before the authoritative save action. */
function normalizeValues(values: CubeEditorMetadataValues): CubeEditorMetadataValues {
  return {
    defaultAlias: values.defaultAlias.trim(),
    targetModel: values.targetModel.trim(),
    supportedModels: values.supportedModels.map((value) => value.trim()).filter(Boolean),
    description: values.description.trim(),
    destination: values.destination,
  };
}

/** Detect the destination of persisted identity without making ownership assumptions. */
function readDestination(cubeId: unknown): CubeEditorMetadataValues['destination'] {
  return typeof cubeId === 'string' && cubeId.trim() && !cubeId.startsWith('local/')
    ? 'pack'
    : 'local';
}

/** Read one trimmed metadata string from a graph-owned record. */
function readString(value: unknown): string {
  return typeof value === 'string' ? value.trim() : '';
}

/** Read stable, non-empty metadata strings without retaining dynamic values. */
function readStringArray(value: unknown): string[] {
  if (!Array.isArray(value)) return [];
  return value
    .filter((entry): entry is string => typeof entry === 'string')
    .map((entry) => entry.trim())
    .filter(Boolean);
}

/** Preserve actionable errors from the authoritative save workflow. */
function readErrorMessage(error: unknown): string {
  return error instanceof Error ? error.message : 'Unable to save this Cube.';
}
