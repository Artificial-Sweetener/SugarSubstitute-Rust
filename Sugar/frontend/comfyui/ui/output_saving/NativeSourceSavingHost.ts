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
/** Bind workflow source controls to cached topology and server-qualified saver capabilities. */
import type { OutputSavingFeature } from './OutputSavingFeature.js';
import type { SavingNodeCapability } from './OutputSavingCapabilitiesClient.js';
import {
  ComfySourceSavingDocument,
  type SourceSavingDocumentOptions,
} from './ComfySourceSavingDocument.js';
import { NativeSavePrefixFields } from './NativeSavePrefixFields.js';

/** Keep source UI independent from mounted image resources and native graph rendering. */
export class NativeSourceSavingHost {
  #capabilities: ReadonlyMap<string, SavingNodeCapability> = new Map();
  #stop: (() => void) | null = null;
  #epoch = 0;
  #started = false;
  #fields: NativeSavePrefixFields | null = null;
  readonly #stops: (() => void)[] = [];
  /** Bind the presentation feature and existing active-workflow adapters. */
  constructor(
    private readonly feature: OutputSavingFeature,
    private readonly options: Omit<SourceSavingDocumentOptions, 'capabilities'>,
    private readonly report: (error: unknown) => void,
    private readonly surfaces: {
      readonly document: Document;
      subscribe(listener: () => void): () => void;
    },
  ) {}

  /** Qualify types once and lease a provider that reads cached native source handles. */
  setup(): void {
    if (this.#started) return;
    this.#started = true;
    this.#fields = new NativeSavePrefixFields(this.surfaces.document);
    const refresh = () => {
      try {
        this.#fields?.reconcile(
          this.options
            .sources()
            .filter((source) => this.#capabilities.get(String(source.node.type))?.managed === true),
          this.feature.configuration.read()?.preferences.manage_workflow === true,
        );
      } catch (error: unknown) {
        this.report(error);
      }
    };
    this.#stops.push(
      this.feature.configuration.subscribe(refresh),
      this.surfaces.subscribe(refresh),
    );
    const document = new ComfySourceSavingDocument({
      ...this.options,
      capabilities: () => this.#capabilities,
    });
    const epoch = ++this.#epoch;
    void this.feature.capabilities
      .load()
      .then((capabilities) => {
        if (epoch === this.#epoch) {
          this.#capabilities = capabilities;
          this.#stop = this.feature.controls?.bind(document) ?? null;
          refresh();
        }
      })
      .catch(this.report);
  }

  /** Release access without retiring workflow results, settings or other windows. */
  dispose(): void {
    this.#epoch++;
    this.#started = false;
    for (const stop of this.#stops.splice(0)) stop();
    this.#fields?.dispose();
    this.#fields = null;
    this.#stop?.();
    this.#stop = null;
    this.#capabilities = new Map();
  }
}
