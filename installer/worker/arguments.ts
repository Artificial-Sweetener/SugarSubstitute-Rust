// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
/** Read only the explicit worker capability arguments supplied by the native owner. */
export function argument(name: 'workspace' | 'port' | 'token'): string {
  const flag = `--setup-${name}`
  const matches = process.argv.map((value, index) => value === flag ? index : -1).filter(index => index >= 0)
  if (matches.length !== 1) throw new Error(`Exactly one ${flag} is required`)
  const value = process.argv[matches[0]! + 1]
  if (!value || value.startsWith('--') || value.includes('\0')) throw new Error(`Invalid ${flag}`)
  return value
}
