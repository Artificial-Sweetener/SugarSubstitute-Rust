// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
/** Inspect PE imports through the explicitly selected LLVM reader, without loading any image. */
import path from 'node:path'
import { spawnSync } from 'node:child_process'
import { regularArtifact } from './artifact-files.mjs'

const machines = { 'windows-x86_64': 'IMAGE_FILE_MACHINE_AMD64', 'windows-aarch64': 'IMAGE_FILE_MACHINE_ARM64' }
const systemLibraries = new Set([
  'advapi32.dll', 'ntdll.dll', 'kernel32.dll', 'bcryptprimitives.dll', 'user32.dll',
  'ole32.dll', 'oleaut32.dll', 'winhttp.dll',
  // Windows Sockets: https://learn.microsoft.com/windows/win32/api/winsock/nf-winsock-wsastartup
  'ws2_32.dll',
  'dwmapi.dll', 'comdlg32.dll', 'gdi32.dll', 'winmm.dll', 'uxtheme.dll', 'comctl32.dll',
  'combase.dll', 'uiautomationcore.dll', 'dwrite.dll', 'imm32.dll', 'd2d1.dll',
  'shell32.dll', 'shlwapi.dll', 'rstrtmgr.dll', 'iphlpapi.dll', 'bcrypt.dll',
  'powrprof.dll', 'version.dll',
  // Native canvas composition: Direct3D 11, DXGI, DirectComposition, process status, WinRT and shell scaling API sets.
  'd3d11.dll', 'dxgi.dll', 'dcomp.dll', 'psapi.dll',
  'api-ms-win-core-winrt-l1-1-0.dll', 'api-ms-win-shcore-scaling-l1-1-1.dll',
  // Node uses Windows certificate, crash diagnostic and user-profile APIs.
  'crypt32.dll', 'dbghelp.dll', 'userenv.dll',
  'api-ms-win-core-synch-l1-2-0.dll',
  ...['math', 'runtime', 'stdio', 'locale', 'heap', 'string', 'convert'].map(name => `api-ms-win-crt-${name}-l1-1-0.dll`),
])

/** Changes to the supported OS dependency set require release review, not PATH discovery. */
export const isSystemLibrary = name => systemLibraries.has(name)

/** These executables run before any package extraction or generation-specific DLL lookup. */
export async function requireBootstrapImages(selection, inspect) {
  const images = []
  for (const [name, file] of selection.files) {
    if (!name.endsWith('.exe')) continue
    const image = await inspect(file)
    images.push({ name, ...image })
  }
  requireBootstrapDependencies(images)
  return images
}

/** Apply the same startup requirement to already-inspected package entry points. */
export function requireBootstrapDependencies(images) {
  if (!images.length) throw new Error('Native build has no bootstrap executable')
  for (const image of images) {
    if (image.imports.some(value => !isSystemLibrary(value.library))) throw new Error(`Native entry requires a separately installed library: ${image.name}`)
  }
}

/** Windows module names cannot select paths, alternate streams or command-line options. */
export function libraryName(value) {
  if (!/^[a-zA-Z0-9_][a-zA-Z0-9_.-]*\.dll$/i.test(value)) throw new Error('Invalid native library name')
  return value.toLowerCase()
}

/** LLVM 19 emits top-level Import/DelayImport/Export blocks; nested symbols remain indented. */
export function parseImage(text, platform) {
  const machine = machines[platform]
  if (!machine) throw new Error('Unsupported native package platform')
  const observed = [...text.matchAll(/^\s+Machine: (\S+) \(0x[0-9A-Fa-f]+\)\s*$/gm)]
  if ([...text.matchAll(/^Format: COFF-/gm)].length !== 1 || observed.length !== 1 || observed[0][1] !== machine) {
    throw new Error('Native library architecture differs from package platform')
  }
  const imports = [], exports = []
  const blocks = [...text.matchAll(/^(Import|DelayImport|Export) \{\r?\n([\s\S]*?)^\}/gm)]
  if (blocks.length !== [...text.matchAll(/^(Import|DelayImport|Export) \{/gm)].length) throw new Error('Incomplete native import inspection')
  for (const [, kind, body] of blocks) {
    if (kind === 'Export') {
      const names = [...body.matchAll(/^  Name: ([^\r\n]*)\r?$/gm)]
      const ordinals = [...body.matchAll(/^  Ordinal: (\d+)\r?$/gm)]
      if (names.length !== 1 || ordinals.length !== 1 || Number(ordinals[0][1]) > 0xffffffff) throw new Error('Invalid native export')
      const name = names[0][1], ordinal = Number(ordinals[0][1])
      if (exports.some(value => value.ordinal === ordinal || (name && value.name === name))) throw new Error('Ambiguous native export')
      exports.push({ name, ordinal })
      const forwards = [...body.matchAll(/^  ForwardedTo: (\S+)\r?$/gm)]
      if (forwards.length > 1 || forwards.length !== [...body.matchAll(/^  ForwardedTo:/gm)].length) throw new Error('Invalid forwarded export')
      for (const [, forward] of forwards) {
        const separator = forward.lastIndexOf('.')
        if (separator < 1) throw new Error('Invalid forwarded export')
        const module = forward.slice(0, separator)
        const symbol = forward.slice(separator + 1)
        if (!symbol || (symbol.startsWith('#') && !/^#\d+$/.test(symbol))) throw new Error('Invalid forwarded symbol')
        const binding = {
          library: libraryName(/\.dll$/i.test(module) ? module : `${module}.dll`),
          symbols: symbol.startsWith('#') ? [] : [symbol],
          ordinals: symbol.startsWith('#') ? [Number(symbol.slice(1))] : [],
        }
        exports.at(-1).forward = binding
        imports.push(binding)
      }
    } else {
      const names = [...body.matchAll(/^  Name: ([^\r\n]+)\r?$/gm)]
      if (names.length !== 1) throw new Error('Native import name is absent or ambiguous')
      const symbols = [...body.matchAll(/^\s+Symbol: (.*) \((\d+)\)\r?$/gm)]
      if (symbols.length !== [...body.matchAll(/^\s+Symbol:/gm)].length || symbols.some(value => Number(value[2]) > 65535)) throw new Error('Invalid imported symbol')
      imports.push({ library: libraryName(names[0][1]), symbols: symbols.filter(value => value[1]).map(value => value[1]), ordinals: symbols.filter(value => !value[1]).map(value => Number(value[2])) })
    }
  }
  for (const [directory, kind] of [['ImportTable', 'Import'], ['DelayImportDescriptor', 'DelayImport'], ['ExportTable', 'Export']]) {
    const values = [...text.matchAll(new RegExp(`^\\s+${directory}RVA: (0x[0-9A-Fa-f]+)\\s*$`, 'gm'))]
    if (values.length !== 1 || (Number(values[0][1]) !== 0) !== blocks.some(block => block[1] === kind)) {
      throw new Error('Native import directory was not fully inspected')
    }
  }
  if (imports.some(value => value.ordinals.some(ordinal => !Number.isSafeInteger(ordinal) || ordinal < 1 || ordinal > 65535))) throw new Error('Invalid imported ordinal')
  return { imports, exports }
}

/** Select and version-check a build tool explicitly; never discover a replacement through PATH. */
export async function windowsInspector(executable, platform) {
  if (!path.isAbsolute(executable) || !machines[platform]) throw new Error('Explicit inspector and Windows platform required')
  await regularArtifact(executable)
  const invoke = args => {
    const result = spawnSync(executable, args, { encoding: 'utf8', windowsHide: true, timeout: 30000, maxBuffer: 16 * 1024 ** 2 })
    if (result.error) throw result.error
    if (result.status !== 0 || result.stderr.trim()) throw new Error(`Native inspection failed: ${result.stderr}`)
    return result.stdout
  }
  if (!/^\s*LLVM version 19\.1\.1\s*$/m.test(invoke(['--version']))) throw new Error('Review the native inspector version before packaging')
  return async file => {
    await regularArtifact(file)
    return parseImage(invoke(['--file-header', '--coff-imports', '--coff-exports', file]), platform)
  }
}
