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
/** Retain installed native component definitions independently of graph-bound renderer mounts. */
import {
  loadComfyNativeNodeComponent,
  loadComfyVueRuntime,
  type ComfyVueRuntime,
} from './ComfyRuntimeModuleLoader.js';
import { findComfyFrontendEntry } from './ComfyFrontendModuleGraph.js';
import type { UnknownRecord } from '../types/common.js';

/** Coalesce immutable assets without retaining graph objects, controls, or application contexts. */
export class ComfyNativeNodeAssets {
  #component: Promise<UnknownRecord> | null = null;
  #runtime: Promise<ComfyVueRuntime> | null = null;

  /** Inject installed asset acquisition for deterministic retry and lifetime proofs. */
  constructor(
    private readonly loadComponent: () => Promise<UnknownRecord>,
    private readonly loadRuntime: () => Promise<ComfyVueRuntime>,
  ) {}

  /** Share one native component module for every graph-bound renderer in this application. */
  component(): Promise<UnknownRecord> {
    if (this.#component) return this.#component;
    const pending = this.loadComponent().catch((error: unknown) => {
      if (this.#component === pending) this.#component = null;
      throw error;
    });
    this.#component = pending;
    return pending;
  }

  /** Keep successful native runtime capabilities and allow transport failures to retry. */
  runtime(): Promise<ComfyVueRuntime> {
    if (this.#runtime) return this.#runtime;
    const pending = this.loadRuntime().catch((error: unknown) => {
      if (this.#runtime === pending) this.#runtime = null;
      throw error;
    });
    this.#runtime = pending;
    return pending;
  }
}

const applications = new WeakMap<
  Document,
  { entry: string | null; assets: ComfyNativeNodeAssets }
>();

/** Scope immutable assets to one document and installed entry revision, letting document collection retire them. */
export function comfyNativeNodeAssets(document: Document): ComfyNativeNodeAssets {
  const entry = findComfyFrontendEntry(document);
  const current = applications.get(document);
  if (current?.entry === entry) return current.assets;
  const assets = new ComfyNativeNodeAssets(
    () => loadComfyNativeNodeComponent(document),
    () => loadComfyVueRuntime(document),
  );
  applications.set(document, { entry, assets });
  return assets;
}
