// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
/** Explicit installer settings port; never opens Comfy Desktop's preferences. */
const values = {
  pypiMirror: '',
  useChineseMirrors: false,
  gitMirror: '',
  hfMirror: '',
}
let mirrors = false

export function configure(useMirrors: boolean): void {
  mirrors = useMirrors
}

export function get<Key extends keyof typeof values>(key: Key): (typeof values)[Key]
export function get(key: string): unknown
export function get(key: string): unknown {
  if (key === 'useChineseMirrors') return mirrors
  if (Object.hasOwn(values, key)) return values[key as keyof typeof values]
  throw new Error(`Unsupported provisioning setting: ${key}`)
}

/** Provisioning cannot mutate Desktop preference or beta-notice state. */
export function set(_key: string, _value: unknown): void {
  throw new Error('Desktop preference writes are outside the provisioning boundary')
}

export function getMirrorConfig(): { pypiMirror: string; useChineseMirrors: boolean } {
  return { pypiMirror: '', useChineseMirrors: mirrors }
}
