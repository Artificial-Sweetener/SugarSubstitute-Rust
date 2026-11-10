// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
/** Decode the pinned inspector's regular, delayed and forwarded dependency formats. */
import { test } from 'node:test'
import assert from 'node:assert/strict'
import { parseImage, libraryName, requireBootstrapDependencies } from './windows-imports.mjs'

const header = 'Format: COFF-x86-64\nImageFileHeader {\n  Machine: IMAGE_FILE_MACHINE_AMD64 (0x8664)\n}\nImageOptionalHeader {\n  ExportTableRVA: 0x3000\n  ImportTableRVA: 0x1000\n  DelayImportDescriptorRVA: 0x2000\n}\n'
const imports = 'Import {\n  Name: KERNEL32.dll\n  Symbol: CloseHandle (1)\n}\nDelayImport {\n  Name: runtime.dll\n  Import {\n    Symbol: DeferredCall (0)\n  }\n}\nExport {\n  Ordinal: 1\n  Name: Forwarded\n  ForwardedTo: OtherLibrary.Function\n}\n'

test('setup bootstrap accepts the Windows socket library and rejects external runtimes and lookalikes', () => {
  const socketHeader = header.replace('0x2000', '0x0').replace('0x3000', '0x0')
  const image = name => ({ name: 'sugarsubstitute-setup.exe', ...parseImage(socketHeader + `Import {\n  Name: ${name}\n  Symbol: WSAStartup (1)\n}\n`, 'windows-x86_64') })
  assert.doesNotThrow(() => requireBootstrapDependencies([image('WS2_32.dll')]))
  for (const name of ['vcruntime140.dll', 'ws2_32_helper.dll', 'winsock-custom.dll']) {
    assert.throws(() => requireBootstrapDependencies([image(name)]), /separately installed library/)
  }
})

test('ordinary imports, delayed symbols and forwarded exports all contribute dependencies', () => {
  assert.deepEqual(parseImage(header + imports, 'windows-x86_64').imports.map(value => value.library).sort(), ['kernel32.dll', 'otherlibrary.dll', 'runtime.dll'])
  assert.deepEqual(parseImage(header.replace('AMD64 (0x8664)', 'ARM64 (0xAA64)') + imports, 'windows-aarch64').imports.map(value => value.library).sort(), ['kernel32.dll', 'otherlibrary.dll', 'runtime.dll'])
  const image = parseImage(header + imports, 'windows-x86_64')
  assert.deepEqual(image.imports.find(value => value.library === 'runtime.dll').symbols, ['DeferredCall'])
  assert.deepEqual(image.exports, [{ name: 'Forwarded', ordinal: 1, forward: { library: 'otherlibrary.dll', symbols: ['Function'], ordinals: [] } }])
  assert.deepEqual(parseImage(header + imports.replace('OtherLibrary.Function', 'OtherLibrary.#7'), 'windows-x86_64').imports.find(value => value.library === 'otherlibrary.dll').ordinals, [7])
})

test('ambiguous, truncated, malformed and wrong-architecture inspection fails closed', () => {
  for (const value of [
    header + imports + header,
    header + imports.replace('  Name: runtime.dll\n', ''),
    header + imports.replace('Name: runtime.dll', 'Name: ../runtime.dll'),
    header + imports.replace('Name: runtime.dll', 'Name: runtime.dll\n  Name: other.dll'),
    header + imports.replace('  ForwardedTo: OtherLibrary.Function', '  ForwardedTo: Invalid'),
    header + imports.replace('  ForwardedTo: OtherLibrary.Function', '  ForwardedTo: '),
    header + imports.replace('  ForwardedTo: OtherLibrary.Function', '  ForwardedTo: OtherLibrary.#0'),
    header + imports.slice(0, -2),
    header,
    (header + imports).replace('IMAGE_FILE_MACHINE_AMD64', 'IMAGE_FILE_MACHINE_I386'),
    (header + imports).replace('  DelayImportDescriptorRVA: 0x2000\n', ''),
  ]) assert.throws(() => parseImage(value, 'windows-x86_64'))
  assert.throws(() => parseImage(header + imports, 'linux-x86_64'))
  for (const name of ['../runtime.dll', 'C:runtime.dll', 'runtime.dll/other', '--runtime.dll', 'runtime']) assert.throws(() => libraryName(name))
})

test('an image with no import tables requires no fabricated dependency', () => {
  assert.deepEqual(parseImage(header.replace('0x1000', '0x0').replace('0x2000', '0x0').replace('0x3000', '0x0'), 'windows-x86_64'), { imports: [], exports: [] })
})

test('the host bootstrap accepts the Windows graphics and WinRT system libraries native composition imports', () => {
  const socketHeader = header.replace('0x2000', '0x0').replace('0x3000', '0x0')
  const image = name => ({ name: 'sugarsubstitute.exe', ...parseImage(socketHeader + `Import {
  Name: ${name}
  Symbol: Call (1)
}
`, 'windows-x86_64') })
  for (const name of ['d3d11.dll', 'DXGI.dll', 'dcomp.dll', 'psapi.dll', 'api-ms-win-core-winrt-l1-1-0.dll', 'api-ms-win-shcore-scaling-l1-1-1.dll']) {
    assert.doesNotThrow(() => requireBootstrapDependencies([image(name)]))
  }
  assert.throws(() => requireBootstrapDependencies([image('d3dcompiler_47.dll')]), /separately installed library/)
})
