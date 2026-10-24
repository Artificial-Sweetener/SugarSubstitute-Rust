// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
/** Presentation types carry host-resolved copy and explicit upstream option text, never a locale corpus. */
import type { InstallerMessages } from './messages'
export type { InstallerMessages }
export type Page = 'language' | 'location' | 'runtime' | 'configuration' | 'folders' | 'maintenance' | 'progress' | 'complete'
export type OptionField = 'release' | 'comfyVersion' | 'variant'
export type PathField = 'installRoot' | 'comfyRoot' | 'python' | 'models' | 'outputs'
export interface UpstreamOption { value: string; label: string; recommended: boolean; description?: string; selectionToken?: string }
export interface Existing { selection: { installRoot: string; comfyRoot: string }; executableMissing: boolean; launchEntryMissing: boolean }
export interface Location { existing?: Existing; comfyRoot: string; models: string; outputs: string }
export interface Selection { locale: string; installRoot: string; mode: string; comfyRoot: string; python: string; models: string; outputs: string; selectionToken?: string }
export interface Replies {
  locale: InstallerMessages; location: Location; options: UpstreamOption[];
  runtime: unknown; validate: unknown; install: unknown; maintain: unknown; open: unknown; browse: string | null;
}
export interface Elements {
  locale: HTMLSelectElement; installRoot: HTMLInputElement; comfyRoot: HTMLInputElement;
  python: HTMLInputElement; models: HTMLInputElement; outputs: HTMLInputElement;
  release: HTMLSelectElement; comfyVersion: HTMLSelectElement; variant: HTMLSelectElement;
  close: HTMLButtonElement; primary: HTMLButtonElement; back: HTMLButtonElement;
  details: HTMLDetailsElement; journey: HTMLProgressElement; total: HTMLProgressElement; mirrors: HTMLInputElement;
  step: HTMLElement; maintenanceNotice: HTMLElement; managed: HTMLElement; attached: HTMLElement;
  managedIcon: HTMLElement; attachedIcon: HTMLElement; configurationTitle: HTMLElement;
  configurationDescription: HTMLElement; pages: HTMLElement; wait: HTMLElement; error: HTMLElement;
  variantDetail: HTMLElement; maintenancePath: HTMLElement; maintenanceComfy: HTMLElement;
  status: HTMLElement; installedPath: HTMLElement; brand: HTMLElement; console: HTMLElement;
  language: HTMLElement; location: HTMLElement; runtime: HTMLElement; configuration: HTMLElement;
  folders: HTMLElement; maintenance: HTMLElement; progress: HTMLElement; complete: HTMLElement;
}
export interface HostEvent { id?: string; event: string; backward?: boolean; data: unknown }
declare global {
  interface Window {
    setupBootstrap: { messages: InstallerMessages; locales: { id: string; label: string }[]; locale: string; installRoot: string };
    ipc: { postMessage(value: string): void };
    setupReceive(message: HostEvent): void;
  }
}
