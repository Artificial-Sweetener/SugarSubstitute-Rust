// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
const { contextBridge, ipcRenderer } = require('electron')
contextBridge.exposeInMainWorld('ipc', { postMessage: message => ipcRenderer.send('setup-command', message) })
