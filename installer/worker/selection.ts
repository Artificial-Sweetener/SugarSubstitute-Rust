// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
/** Keep upstream option objects private; the renderer chooses only advertised identities. */
import { randomUUID } from 'node:crypto'
import { standalone } from '@upstream/sources/standalone/index'
import { detectGPU } from '@upstream/lib/gpu'
import { configure } from './settings'
import { init } from '@upstream/lib/i18n'

type Option = { value: string; label: string; description?: string; recommended?: boolean; data?: Record<string, unknown> }
const choices = new Map<string, Option[]>()
const plans = new Map<string, Record<string, unknown>>()
let scope = ''

export async function options(input: {
  field: string; selections?: Record<string, string>; mirrors?: boolean; locale?: string
}): Promise<unknown> {
  const settingKey = JSON.stringify([input.mirrors === true, input.locale ?? 'en'])
  if (settingKey !== scope) { choices.clear(); plans.clear(); scope = settingKey }
  configure(input.mirrors === true)
  init(input.locale ?? 'en')
  const selected: Record<string, Option | undefined> = {}
  const order = ['release', 'comfyVersion', 'variant']
  if (standalone.fields?.map(field => field.id).join(',') !== order.join(',')) {
    throw new Error('Upstream installation fields changed; the provisioning adapter requires review')
  }
  const index = order.indexOf(input.field)
  for (const key of order.slice(0, index)) {
    const picked = input.selections?.[key]
    const options = choices.get(key)
    if (!options) throw new Error('Resolve preceding provisioning options first')
    if (!options.length && key === 'comfyVersion') continue
    const value = options.find(option => option.value === picked)
    if (!value) throw new Error('Selection is no longer available')
    selected[key] = value
  }
  for (const key of order.slice(index)) choices.delete(key)
  plans.clear()
  const gpu = await detectGPU()
  const result: Option[] = await standalone.getFieldOptions!(input.field, selected, {
    includeLatestStable: true, gpu: gpu?.id,
  })
  if (result.length > 256) throw new Error('Upstream option catalogue exceeds its bound')
  choices.set(input.field, result)
  return result.map(option => {
    let selectionToken: string | undefined
    if (input.field === 'variant') {
      selectionToken = randomUUID()
      plans.set(selectionToken, standalone.buildInstallation({ ...selected, variant: option }))
    }
    return {
      value: option.value, label: option.label, description: option.description,
      recommended: option.recommended === true, selectionToken,
    }
  })
}

export function consumePlan(token: string): Record<string, unknown> {
  const plan = plans.get(token)
  if (!plan) throw new Error('Provisioning selection expired; review the options again')
  plans.clear()
  if ('downloadTemplateModels' in plan || 'bundledTemplateId' in plan) {
    throw new Error('Model acquisition is outside this installer')
  }
  return plan
}
