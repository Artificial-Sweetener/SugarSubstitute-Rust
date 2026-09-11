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
/** Compose independent SugarScript and reconciled-workflow PNG import paths. */

import {
  PngSugarScriptArtifactError,
  readPngSugarScriptMetadata,
} from './PngSugarScriptArtifactReader.js';

export interface SugarScriptFileHost {
  handleFile(file: File, ...arguments_: unknown[]): Promise<unknown>;
}

interface SugarScriptImageFeedback {
  push?(severity: string, summary: string, detail?: string): unknown;
}

interface SugarScriptImageFileHostAdapterOptions {
  host: SugarScriptFileHost;
  importSource(source: string): Promise<unknown>;
  importWorkflow(workflow: Record<string, unknown>): Promise<unknown>;
  validateWorkflow(workflow: Record<string, unknown>): Promise<unknown>;
  retainWorkflowSource(source: string): Promise<unknown>;
  feedback?: SugarScriptImageFeedback | null;
  readErrorMessage(error: unknown): string;
}

/** Own one reversible host method adaptation for SugarScript-bearing PNGs. */
export class SugarScriptImageFileHostAdapter {
  readonly #host: SugarScriptFileHost;
  readonly #importSource: (source: string) => Promise<unknown>;
  readonly #importWorkflow: (workflow: Record<string, unknown>) => Promise<unknown>;
  readonly #validateWorkflow: (workflow: Record<string, unknown>) => Promise<unknown>;
  readonly #retainWorkflowSource: (source: string) => Promise<unknown>;
  readonly #feedback: SugarScriptImageFeedback | null;
  readonly #readErrorMessage: (error: unknown) => string;
  readonly #original: SugarScriptFileHost['handleFile'];
  #installed: SugarScriptFileHost['handleFile'] | null = null;
  #active = false;

  /** Bind the existing host loader and native SugarScript import use case. */
  constructor(options: SugarScriptImageFileHostAdapterOptions) {
    this.#host = options.host;
    this.#importSource = options.importSource;
    this.#importWorkflow = options.importWorkflow;
    this.#validateWorkflow = options.validateWorkflow;
    this.#retainWorkflowSource = options.retainWorkflowSource;
    this.#feedback = options.feedback ?? null;
    this.#readErrorMessage = options.readErrorMessage;
    this.#original = options.host.handleFile;
  }

  /** Install idempotently while retaining exact delegation for every other file. */
  setup(): void {
    if (this.#active) return;
    this.#active = true;
    this.#installed = (file, ...arguments_) => this.#handle(file, arguments_);
    this.#host.handleFile = this.#installed;
  }

  /** Restore the exact loader only while this adapter still owns it. */
  dispose(): void {
    if (!this.#active) return;
    this.#active = false;
    if (this.#host.handleFile === this.#installed) {
      this.#host.handleFile = this.#original;
    }
    this.#installed = null;
  }

  /** Validate each attached representation before choosing its declared authority. */
  async #handle(file: File, arguments_: readonly unknown[]): Promise<unknown> {
    if (!isPng(file)) return this.#delegate(file, arguments_);
    let metadata;
    try {
      metadata = await readPngSugarScriptMetadata(file);
    } catch (error: unknown) {
      if (!isRecognizedSugarScriptFailure(error)) {
        return await this.#delegate(file, arguments_);
      }
      this.#reportFailure(error);
      return undefined;
    }
    if (metadata.sugarScript === null && metadata.workflow === null) {
      return this.#delegate(file, arguments_);
    }
    if (metadata.sugarScript === null && isManagedCubeWorkflow(metadata.workflow)) {
      try {
        const result = await this.#importWorkflow(metadata.workflow);
        this.#feedback?.push?.('success', 'Cube workflow imported', file.name);
        return result;
      } catch (error: unknown) {
        return this.#loadEmbeddedWorkflow(file, arguments_, null, error);
      }
    }
    if (metadata.sugarScript === null) return this.#delegate(file, arguments_);

    const managedWorkflow = isManagedCubeWorkflow(metadata.workflow) ? metadata.workflow : null;
    let workflowValidated = false;
    let workflowValidationFailure: unknown;
    if (managedWorkflow) {
      try {
        await this.#validateWorkflow(managedWorkflow);
        workflowValidated = true;
      } catch (error: unknown) {
        workflowValidationFailure = error;
      }
    }
    try {
      const result = await this.#importSource(metadata.sugarScript);
      this.#feedback?.push?.('success', 'SugarScript workflow imported', file.name);
      return result;
    } catch (error: unknown) {
      if (managedWorkflow && workflowValidated) {
        let result: unknown;
        try {
          result = await this.#importWorkflow(managedWorkflow);
        } catch (workflowError: unknown) {
          return this.#loadEmbeddedWorkflow(file, arguments_, metadata.sugarScript, workflowError);
        }
        await this.#retainSourceWithoutBlocking(metadata.sugarScript);
        this.#feedback?.push?.(
          'warning',
          'Workflow imported after SugarScript failed',
          this.#readErrorMessage(error),
        );
        return result;
      }
      if (metadata.workflow !== null) {
        return this.#loadEmbeddedWorkflow(
          file,
          arguments_,
          metadata.sugarScript,
          workflowValidationFailure ?? error,
        );
      }
      this.#reportFailure(error);
      return undefined;
    }
  }

  /** Load attached workflow bytes when optional Cube reconstruction is unavailable. */
  async #loadEmbeddedWorkflow(
    file: File,
    arguments_: readonly unknown[],
    source: string | null,
    reconstructionFailure: unknown,
  ): Promise<unknown> {
    try {
      const result = await this.#delegate(file, arguments_);
      if (source !== null) await this.#retainSourceWithoutBlocking(source);
      this.#feedback?.push?.(
        'warning',
        'Embedded workflow loaded after reconstruction failed',
        this.#readErrorMessage(reconstructionFailure),
      );
      return result;
    } catch (error: unknown) {
      this.#reportFailure(error, 'Recipe image import failed');
      return undefined;
    }
  }

  /** Preserve optional SugarScript without making a usable workflow fail. */
  async #retainSourceWithoutBlocking(source: string): Promise<void> {
    try {
      await this.#retainWorkflowSource(source);
    } catch {
      // The workflow remains authoritative when optional source retention fails.
    }
  }

  /** Present one recognized metadata or compilation failure without host fallback. */
  #reportFailure(error: unknown, summary = 'SugarScript image import failed'): void {
    this.#feedback?.push?.('error', summary, this.#readErrorMessage(error));
  }

  /** Preserve the original receiver and all evolving host arguments. */
  #delegate(file: File, arguments_: readonly unknown[]): Promise<unknown> {
    return Reflect.apply(this.#original, this.#host, [file, ...arguments_]) as Promise<unknown>;
  }
}

/** Narrow an evolving Comfy application to the stable file-loader capability. */
export function isSugarScriptFileHost(value: unknown): value is SugarScriptFileHost {
  return typeof (value as Partial<SugarScriptFileHost> | null)?.handleFile === 'function';
}

/** Recognize PNGs before invoking the strict artifact reader. */
function isPng(file: File): boolean {
  return (
    file.type.toLocaleLowerCase() === 'image/png' || file.name.toLocaleLowerCase().endsWith('.png')
  );
}

/** Recognize only group-era workflows owned by SugarCubes. */
function isManagedCubeWorkflow(
  workflow: Record<string, unknown> | null,
): workflow is Record<string, unknown> {
  if (!workflow || !Array.isArray(workflow.groups)) return false;
  return workflow.groups.some((group) => {
    if (typeof group !== 'object' || group === null || Array.isArray(group)) return false;
    const metadata = (group as Record<string, unknown>).sugarcubes;
    if (typeof metadata !== 'object' || metadata === null || Array.isArray(metadata)) return false;
    const values = metadata as Record<string, unknown>;
    return (
      values.managed !== false &&
      typeof values.cube_id === 'string' &&
      typeof values.cube_version === 'string'
    );
  });
}

/** Consume only failures that prove SugarScript metadata was present. */
function isRecognizedSugarScriptFailure(error: unknown): boolean {
  if (!(error instanceof PngSugarScriptArtifactError)) return false;
  return (
    error.code === 'image.duplicate_sugarscript' ||
    error.code === 'image.duplicate_workflow' ||
    error.code === 'image.source_too_large' ||
    error.code === 'image.workflow_too_large' ||
    error.code === 'image.invalid_workflow' ||
    error.message.includes("'sugar_script'") ||
    error.message.includes("'workflow'")
  );
}
