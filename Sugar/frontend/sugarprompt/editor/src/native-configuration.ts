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
/** One native configuration owner captures host values and owns every reconfiguration compartment. */
import { requireValue } from '../../core/src/required.js';
import { validateConfigurationValues } from './host-configuration-validation.js';
import { Compartment, type Extension, type StateEffect } from '@codemirror/state';
import { placeholder } from '@codemirror/view';
import type { EditorOptions } from './editor-options.js';
import type { HostConfiguration } from './host-update.js';
import { captureConditioningContext, type ConditioningContext } from '../../services/src/index.js';
import { conditioningContext } from './conditioning-context.js';
import { regionPalette } from './region-paint.js';
import { formatRegionHex, parseRegionHex } from '../../core/src/index.js';
import { documentKind } from './document-kind.js';
import { loraPageHost, loraMetadataHost } from './lora-action-hosts.js';
import { captureLoraSchedulingHost, loraSchedulingHost } from './lora-schedule.js';
import {
  tagCompletionCatalog,
  loraCompletionCatalog,
  sceneTitleCatalog,
} from './prompt-completion-model.js';
import { wildcardCatalog } from './wildcard-catalog.js';
import { booruPostLookup } from './booru-paste-host.js';
import { spellingSession } from './spelling-diagnostics.js';
import { sceneQueueHost } from './scene-queue.js';
import { sourceLineChrome } from './source-lines.js';
import { wheelMode, initialWheelMode } from './wheel-mode.js';
import { richRendering } from './presentation.js';
import { featureProfile } from './features.js';
import { invalidateCommandContext } from './freshness.js';
import { savedSegmentHost } from './saved-segment-menu.js';
import { captureFieldActionUtilities, fieldActionUtilities } from './field-action-utilities.js';
/** Fully captured final values; no compartment or mutable buffer is exposed to consumers. */
export type NativeConfigurationValues = Required<
  Omit<HostConfiguration, 'readOnly' | 'tagLexicon' | 'featureProfile' | 'conditioningContext'>
> &
  Pick<EditorOptions, 'tagLexicon' | 'featureProfile'> & {
    readonly conditioningContext: ConditioningContext;
  };
/** Prepared configuration is inert until the native coordinator accepts it. */
export interface PreparedNativeConfiguration {
  readonly values: NativeConfigurationValues;
  readonly effects: readonly StateEffect<unknown>[];
  /** Equality-owned changed fields accompany the actual prepared effects, without deciding history policy. */
  readonly changedKeys: readonly (keyof HostConfiguration)[];
}
/** One responsibility owns captured values, validation and supported native compartments. */
export class NativeConfiguration {
  private readonly compartments = new Map<keyof NativeConfigurationValues, Compartment>();
  private current: NativeConfigurationValues;
  /** Validate mount intent before the first native state exists. */
  constructor(options: EditorOptions) {
    this.current = this.capture({
      documentKind: 'prompt',
      savedSegmentHost: null,
      fieldActionUtilities: null,
      conditioningContext: null,
      regionAccent: null,
      richRendering: true,
      sourceLineChrome: { enabled: false, contentLeftInset: 0 },
      wheelMode: 'hover_dwell',
      tagCompletionCatalog: null,
      loraCompletionCatalog: null,
      wildcardCatalog: null,
      booruPostLookup: null,
      spellcheckSession: null,
      sceneTitleCatalog: null,
      sceneQueueHost: null,
      loraPageHost: null,
      loraMetadataHost: null,
      loraSchedulingHost: { context: null, catalogRevision: null },
      ...options,
    });
    for (const key of [
      ...Object.keys(this.current),
      'featureProfile',
    ] as (keyof NativeConfigurationValues)[])
      if (key !== 'tagLexicon') this.compartments.set(key, new Compartment());
  }
  /** Resolve accepted configuration for derived consumers and public adapters. */
  get values(): NativeConfigurationValues {
    return this.current;
  }
  /** Validate and capture values without changing native authority or prior accepted identities. */
  private capture(input: HostConfiguration): NativeConfigurationValues {
    validateConfigurationValues(input);
    const values = {
      documentKind: requireValue(input.documentKind, 'native-configuration'),
      savedSegmentHost: input.savedSegmentHost ?? null,
      fieldActionUtilities: input.fieldActionUtilities ?? null,
      conditioningContext: captureConditioningContext(input.conditioningContext),
      regionAccent: captureRegionAccent(input.regionAccent ?? null),
      placeholder: requireValue(input.placeholder, 'native-configuration'),
      richRendering: requireValue(input.richRendering, 'native-configuration'),
      sourceLineChrome: Object.freeze({
        ...requireValue(input.sourceLineChrome, 'native-configuration'),
      }),
      wheelMode: initialWheelMode(input.wheelMode),
      tagCompletionCatalog: input.tagCompletionCatalog ?? null,
      loraCompletionCatalog: input.loraCompletionCatalog ?? null,
      wildcardCatalog: input.wildcardCatalog ?? null,
      booruPostLookup: input.booruPostLookup ?? null,
      spellcheckSession: input.spellcheckSession ?? null,
      sceneTitleCatalog: input.sceneTitleCatalog ?? null,
      sceneQueueHost: input.sceneQueueHost ?? null,
      loraPageHost: input.loraPageHost ?? null,
      loraMetadataHost: input.loraMetadataHost ?? null,
      loraSchedulingHost: captureLoraSchedulingHost(
        requireValue(input.loraSchedulingHost, 'native-configuration'),
      ),
      ...(input.featureProfile ? { featureProfile: input.featureProfile } : {}),
      ...(input.tagLexicon ? { tagLexicon: input.tagLexicon } : {}),
    };
    return Object.freeze(values);
  }
  /** Map each accepted domain value to its one supported native extension. */
  private extension(
    key: keyof NativeConfigurationValues,
    values: NativeConfigurationValues,
    revision: number,
  ): Extension {
    switch (key) {
      case 'fieldActionUtilities':
        return fieldActionUtilities.of(captureFieldActionUtilities(values.fieldActionUtilities));
      case 'savedSegmentHost':
        return savedSegmentHost.of(values.savedSegmentHost);
      case 'documentKind':
        return documentKind.of(values.documentKind);
      case 'conditioningContext':
        return conditioningContext.of(values.conditioningContext);
      case 'regionAccent':
        return values.regionAccent === null ? [] : regionPalette.of(values.regionAccent);
      case 'placeholder':
        return values.placeholder === '' ? [] : placeholder(values.placeholder);
      case 'richRendering':
        return richRendering.of(values.richRendering);
      case 'sourceLineChrome':
        return sourceLineChrome.of(values.sourceLineChrome);
      case 'wheelMode':
        return wheelMode.of(values.wheelMode);
      case 'featureProfile':
        return values.featureProfile ? featureProfile.of(values.featureProfile) : [];
      case 'tagCompletionCatalog':
        return tagCompletionCatalog.of(values.tagCompletionCatalog);
      case 'loraCompletionCatalog':
        return loraCompletionCatalog.of(values.loraCompletionCatalog);
      case 'wildcardCatalog':
        return wildcardCatalog.of(values.wildcardCatalog);
      case 'booruPostLookup':
        return booruPostLookup.of(values.booruPostLookup);
      case 'spellcheckSession':
        return spellingSession.of(values.spellcheckSession);
      case 'sceneTitleCatalog':
        return sceneTitleCatalog.of(values.sceneTitleCatalog);
      case 'sceneQueueHost':
        return sceneQueueHost.of(
          values.sceneQueueHost?.capability.sourceRevision === revision
            ? values.sceneQueueHost
            : null,
        );
      case 'loraPageHost':
        return loraPageHost.of(values.loraPageHost);
      case 'loraMetadataHost':
        return loraMetadataHost.of(values.loraMetadataHost);
      case 'loraSchedulingHost':
        return loraSchedulingHost.of(values.loraSchedulingHost);
      case 'tagLexicon':
        return [];
    }
  }
  /** Baseline construction uses final captured values without committing them. */
  extensions(values = this.current, revision = 0): readonly Extension[] {
    return [...this.compartments].map(([key, compartment]) =>
      compartment.of(this.extension(key, values, revision)),
    );
  }
  /** Reuse actual unchanged profile/catalog/context publications and prepare one effect list. */
  prepare(patch: HostConfiguration, revision: number): PreparedNativeConfiguration {
    if (!patch || typeof patch !== 'object' || Array.isArray(patch))
      throw new TypeError('Invalid configuration');
    const allowed = [...this.compartments.keys(), 'tagLexicon', 'readOnly'];
    if (Object.keys(patch).some((key) => !allowed.includes(key as keyof NativeConfigurationValues)))
      throw new TypeError('Invalid configuration field');
    if (Object.values(patch).some((value) => value === undefined))
      throw new TypeError('Invalid configuration value');
    const captured = this.capture({
      ...this.current,
      conditioningContext:
        this.current.conditioningContext.key === null
          ? null
          : {
              key: this.current.conditioningContext.key,
              mode: this.current.conditioningContext.mode,
            },
      ...patch,
    });
    const values = Object.freeze({
      ...captured,
      conditioningContext:
        captured.conditioningContext.key === this.current.conditioningContext.key &&
        captured.conditioningContext.mode === this.current.conditioningContext.mode
          ? this.current.conditioningContext
          : captured.conditioningContext,
      sourceLineChrome:
        captured.sourceLineChrome.enabled === this.current.sourceLineChrome.enabled &&
        captured.sourceLineChrome.contentLeftInset ===
          this.current.sourceLineChrome.contentLeftInset
          ? this.current.sourceLineChrome
          : captured.sourceLineChrome,
      loraSchedulingHost:
        captured.loraSchedulingHost.context === this.current.loraSchedulingHost.context &&
        captured.loraSchedulingHost.catalogRevision ===
          this.current.loraSchedulingHost.catalogRevision
          ? this.current.loraSchedulingHost
          : captured.loraSchedulingHost,
    });
    const effects: StateEffect<unknown>[] = [];
    const changedKeys: (keyof HostConfiguration)[] = [];
    for (const key of Object.keys(patch) as (keyof NativeConfigurationValues)[]) {
      if (key === 'tagLexicon') {
        if (values.tagLexicon !== this.current.tagLexicon) {
          changedKeys.push(key);
          effects.push(invalidateCommandContext.of(null));
        }
        continue;
      }
      const compartment = this.compartments.get(key);
      if (!compartment) continue;
      const before = this.current[key],
        after = values[key];
      const equal =
        key === 'sourceLineChrome'
          ? this.current.sourceLineChrome.enabled === values.sourceLineChrome.enabled &&
            this.current.sourceLineChrome.contentLeftInset ===
              values.sourceLineChrome.contentLeftInset
          : key === 'conditioningContext'
            ? before === after
            : key === 'loraSchedulingHost'
              ? this.current.loraSchedulingHost.context === values.loraSchedulingHost.context &&
                this.current.loraSchedulingHost.catalogRevision ===
                  values.loraSchedulingHost.catalogRevision
              : before === after;
      if (!equal) {
        changedKeys.push(key);
        effects.push(compartment.reconfigure(this.extension(key, values, revision)));
        if (
          [
            'tagCompletionCatalog',
            'savedSegmentHost',
            'fieldActionUtilities',
            'loraCompletionCatalog',
            'wildcardCatalog',
            'booruPostLookup',
            'sceneTitleCatalog',
            'loraSchedulingHost',
          ].includes(key)
        )
          effects.push(invalidateCommandContext.of(null));
      }
    }
    return { values, effects, changedKeys: Object.freeze(changedKeys) };
  }
  /** Commit only an admitted preparation immediately before its native publication. */
  accept(prepared: PreparedNativeConfiguration): void {
    this.current = prepared.values;
  }
}

/** Normalize a host accent to lowercase `#rrggbb`; malformed accents reject. */
function captureRegionAccent(value: string | null): string | null {
  if (value === null) return null;
  const parsed = typeof value === 'string' ? parseRegionHex(value) : null;
  if (!parsed) throw new TypeError('Invalid region accent');
  return formatRegionHex(parsed);
}
