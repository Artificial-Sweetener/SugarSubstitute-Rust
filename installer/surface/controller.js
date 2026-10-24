// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
(() => {
  'use strict';
  /** @typedef {import('./contracts').InstallerMessages} InstallerMessages */
  /** @typedef {import('./contracts').Page} Page */
  /** @typedef {import('./contracts').Elements} Elements */
  /** @typedef {import('./contracts').OptionField} OptionField */
  /** @template {keyof Elements} K @param {K} id @returns {Elements[K]} */
  const $ = id => /** @type {Elements[K]} */ (document.getElementById(id));
  /** @param {string} selector @returns {NodeListOf<HTMLElement>} */
  const all = selector => document.querySelectorAll(selector);
  const activeMode = () => /** @type {HTMLInputElement} */ (document.querySelector('input[name=mode]:checked'));
  const summary = () => /** @type {HTMLElement} */ ($('details').querySelector('summary'));
  /** @param {string | undefined} key */
  function label(key) {
    if (!key || !Object.hasOwn(copy, key)) throw new Error('Unknown installer message key');
    return copy[/** @type {keyof InstallerMessages} */ (key)];
  }
  /** @param {string | undefined} field @returns {import('./contracts').PathField} */
  function pathField(field) {
    if (!field || !['installRoot', 'comfyRoot', 'python', 'models', 'outputs'].includes(field))
      throw new Error('Unknown installer path field');
    return /** @type {import('./contracts').PathField} */ (field);
  }
  const bootstrap = window.setupBootstrap;
  let copy = bootstrap.messages;
  class ProductError extends Error {}
  // The host returns localized copy; unexpected renderer failures remain diagnostic evidence.
  /** @param {unknown} value @returns {InstallerMessages} */
  function acceptCopy(value) {
    const record = /** @type {Record<string, unknown>} */ (value);
    const keys = Object.keys(bootstrap.messages);
    if (!value || typeof value !== 'object' || Object.keys(value).length !== keys.length ||
        keys.some(key => typeof record[key] !== 'string' || !record[key] || record[key].includes('\0'))) {
      throw new Error('Invalid localized installer contract');
    }
    return /** @type {InstallerMessages} */ (value);
  }
  /** @type {Page} */
  let page = 'language';
  let busy = false;
  let sequence = 0;
  let started = 0;
  let maximum = 0;
  let failed = false;
  let optionsValid = false;
  let unavailable = false;
  /** @type {import('./contracts').Existing | null} */
  let existing = null;
  /** @type {Map<string, {resolve(data: unknown): void; reject(cause: Error): void}>} */
  const pending = new Map();
  /** @type {Page[]} */
  const pages = ['language', 'location', 'runtime', 'configuration', 'folders', 'maintenance', 'progress', 'complete'];
  /** @returns {Page[]} */
  const route = () => existing ? ['language', 'location', 'maintenance', 'progress', 'complete'] :
    ['language', 'location', 'runtime', 'configuration', 'folders', 'progress', 'complete'];
  /** @type {Record<Page, keyof InstallerMessages>} */
  const stageLabels = { language: 'stepLanguage', location: 'stepLocation', runtime: 'stepRuntime',
    configuration: 'stepConfiguration', folders: 'stepFolders', maintenance: 'stepMaintenance',
    progress: 'stepProgress', complete: 'stepComplete' };
  /** @type {Map<OptionField, import('./contracts').UpstreamOption[]>} */
  const options = new Map();
  /** @param {string} operation @param {unknown} [input] */
  const send = (operation, input = {}) => {
    const id = String(++sequence);
    window.ipc.postMessage(JSON.stringify({ version: 1, id, operation, input }));
    return id;
  };
  /** @template {keyof import('./contracts').Replies} T
   * @param {T} operation @param {unknown} input @returns {Promise<import('./contracts').Replies[T]>} */
  function request(operation, input) {
    return new Promise((resolve, reject) => {
      const id = String(++sequence);
      pending.set(id, { resolve: data => resolve(/** @type {import('./contracts').Replies[T]} */ (data)), reject });
      window.ipc.postMessage(JSON.stringify({ version: 1, id, operation, input }));
    });
  }
  function translate() {
    for (const element of all('[data-text]')) element.textContent = label(element.dataset.text);
    for (const element of all('[data-label]')) element.setAttribute('aria-label', label(element.dataset.label));
    document.title = copy.title;
    const stages = route();
    const index = stages.indexOf(page);
    $('step').textContent = copy.step.replace('{current}', String(index + 1)).replace('{total}', String(stages.length)) + ' · ' + copy[stageLabels[page]];
    $('maintenanceNotice').textContent = copy[existing?.launchEntryMissing ? 'maintenanceLaunchMissing' : existing?.executableMissing ? 'maintenanceMissing' : 'maintenanceDescription'];
    renderMode();
    summary().textContent = copy[$('details').open ? 'hideDetails' : 'details'];
  }
  function renderMode() {
    const managed = activeMode().value === 'managed';
    $('managed').hidden = !managed; $('attached').hidden = managed;
    $('managedIcon').hidden = !managed; $('attachedIcon').hidden = managed;
    $('configurationTitle').textContent = copy[managed ? 'configurationTitleManaged' : 'configurationTitleAttached'];
    $('configurationDescription').textContent = copy[managed ? 'configurationDescriptionManaged' : 'configurationDescriptionAttached'];
    for (const label of all('[data-choice-label]')) {
      label.textContent = copy[(label.dataset.choiceLabel === 'managed') === managed ? 'selected' : 'select'];
    }
  }
  /** @param {Page} next */
  function show(next) {
    page = next;
    $('pages').dataset.page = next;
    const stages = route();
    for (const stage of pages) $(stage).hidden = stage !== next;
    $('journey').max = stages.length;
    $('journey').value = stages.indexOf(page) + 1;
    $('back').hidden = ['language', 'progress', 'complete'].includes(page);
    $('primary').hidden = page === 'progress';
    translate();
    $('primary').textContent = copy[page === 'folders' ? 'install' : page === 'maintenance' ? 'maintain' : page === 'complete' ? 'open' : 'continue'];
    $('pages').scrollTop = 0;
    setBusy(false);
  }
  /** @param {boolean} value */
  function setBusy(value) {
    if (value && !busy) started = performance.now();
    busy = value;
    $('primary').disabled = value || unavailable || (page === 'configuration' &&
      activeMode().value === 'managed' && !optionsValid);
    $('back').disabled = value;
    for (const element of /** @type {NodeListOf<HTMLInputElement | HTMLButtonElement | HTMLSelectElement>} */ (document.querySelectorAll('#location input, #location button, #runtime input, #configuration input, #configuration select, #configuration button'))) {
      element.disabled = value;
    }
    updateWait();
  }
  function updateWait() {
    $('wait').hidden = unavailable || failed || (!busy && page !== 'progress') ||
      performance.now() - started < 120000;
  }
  /** @param {unknown} cause */
  function error(cause) {
    $('error').textContent = cause instanceof ProductError ? cause.message : copy.setupFailed;
    if (!(cause instanceof ProductError)) send('rendererDiagnostic', { message: String(cause).slice(0, 1024) });
    $('error').hidden = false;
    setBusy(false);
  }
  function clearError() { $('error').hidden = true; $('error').textContent = ''; }
  function selected() {
    return Object.fromEntries(/** @type {OptionField[]} */ (['release', 'comfyVersion']).filter(key => $(key).value).map(key => [key, $(key).value]));
  }
  /** @param {OptionField} field */
  async function loadOptions(field) {
    optionsValid = false;
    setBusy(true);
    try {
      const values = await request('options', {
        field, selections: selected(), locale: $('locale').value, mirrors: $('mirrors').checked,
      });
      options.set(field, values);
      $(field).replaceChildren(...values.map(value => {
        const element = new Option(value.label, value.value);
        element.selected = value.recommended === true;
        return element;
      }));
      $(field).disabled = !values.length;
      if (field === 'release') await loadOptions('comfyVersion');
      else if (field === 'comfyVersion') await loadOptions('variant');
      else {
        if (!values.length) throw new ProductError(copy.noEnvironment);
        optionsValid = true;
        variantDetail();
        setBusy(false);
      }
    } catch (cause) { error(cause); }
  }
  function variantDetail() {
    $('variantDetail').textContent = options.get('variant')?.find(item => item.value === $('variant').value)?.description ?? '';
  }
  /** @returns {import('./contracts').Selection} */
  function selection() {
    return {
      locale: $('locale').value,
      installRoot: $('installRoot').value,
      mode: activeMode().value,
      comfyRoot: $('comfyRoot').value, python: $('python').value,
      models: $('models').value, outputs: $('outputs').value,
      selectionToken: options.get('variant')?.find(item => item.value === $('variant').value)?.selectionToken,
    };
  }
  $('primary').onclick = async () => {
    if (busy || unavailable) return;
    if (failed) {
      failed = false; clearError();
      if (existing) { show('maintenance'); return; }
      show('configuration');
      if (selection().mode === 'managed') await loadOptions('release');
      return;
    }
    clearError(); setBusy(true);
    try {
      if (page === 'language') {
        copy = acceptCopy(await request('locale', { locale: $('locale').value }));
        document.documentElement.lang = $('locale').value;
        show('location');
      } else if (page === 'location') {
        const defaults = await request('location', { directory: $('installRoot').value });
        existing = defaults.existing ?? null;
        if (existing) {
          $('maintenancePath').textContent = existing.selection.installRoot;
          $('maintenanceComfy').textContent = existing.selection.comfyRoot;
          show('maintenance');
          return;
        }
        for (const key of /** @type {const} */ (['models', 'outputs', 'comfyRoot'])) $(key).value = defaults[key];
        show('runtime');
      } else if (page === 'runtime') {
        show('configuration');
        if (selection().mode === 'managed') await loadOptions('release');
      } else if (page === 'configuration') {
        await request('runtime', selection());
        show('folders');
      } else if (page === 'folders' || page === 'maintenance') {
        const maintaining = page === 'maintenance';
        const installation = maintaining && existing ? { directory: existing.selection.installRoot } : selection();
        if (!maintaining) await request('validate', installation);
        show('progress');
        started = performance.now(); maximum = 0; $('total').value = 0;
        $('status').textContent = copy.preparingInstallation;
        await request(maintaining ? 'maintain' : 'install', installation);
        $('installedPath').textContent = 'directory' in installation ? installation.directory : installation.installRoot;
        show('complete');
      } else if (page === 'complete') {
        await request('open', {});
        send('close');
      }
    } catch (cause) {
      error(cause);
      if (page === 'progress') {
        $('primary').hidden = false;
        $('primary').textContent = copy.retry;
        failed = true;
      }
    }
  };
  $('back').onclick = () => { const stages = route(); clearError(); show(stages[stages.indexOf(page) - 1]); };
  $('locale').onchange = async () => {
    clearError();
    $('locale').disabled = true;
    setBusy(true);
    try {
      copy = acceptCopy(await request('locale', { locale: $('locale').value }));
      document.documentElement.lang = $('locale').value;
      translate();
    } catch (cause) { error(cause); }
    finally { $('locale').disabled = false; setBusy(false); }
  };
  for (const element of all('[data-browse]')) element.onclick = async () => {
    try {
      const field = pathField(element.dataset.browse);
      const path = await request('browse', { field, current: $(field).value });
      if (path) $(field).value = path;
    } catch (cause) { error(cause); }
  };
  for (const radio of all('input[name=mode]')) radio.onchange = () => {
    renderMode();
    setBusy(false);
  };
  $('release').onchange = () => { clearError(); void loadOptions('comfyVersion'); };
  $('comfyVersion').onchange = () => { clearError(); void loadOptions('variant'); };
  $('variant').onchange = variantDetail;
  $('mirrors').onchange = () => { clearError(); void loadOptions('release'); };
  $('close').onclick = () => send('close');
  $('brand').onpointerdown = event => { if (!(event.target instanceof Element) || !event.target.closest('button')) send('drag'); };
  // The native splash console owns text, selection, copy and its sole scrollbar.
  // This placeholder reports placement only; it never contains a second document.
  function consoleLayout() {
    const bounds = $('console').getBoundingClientRect();
    const visible = page === 'progress' && $('details').open && bounds.width > 8 && bounds.height > 8;
    send('console', {
      visible, x: visible ? bounds.x + 4 : 0, y: visible ? bounds.y + 4 : 0,
      width: visible ? bounds.width - 8 : 0, height: visible ? bounds.height - 8 : 0,
    });
  }
  new ResizeObserver(consoleLayout).observe($('console'));
  $('details').addEventListener('toggle', () => {
    summary().textContent = copy[$('details').open ? 'hideDetails' : 'details'];
    consoleLayout();
  });
  window.setupReceive = message => {
    if (message.event === 'console_focus_exit') {
      (message.backward ? summary() : $('close')).focus();
      return;
    }
    if (message.event === 'unavailable') {
      unavailable = true;
      for (const waiting of pending.values()) waiting.reject(new ProductError(copy.workerStopped));
      pending.clear();
      error(new ProductError(copy.workerStopped));
      show('progress');
      $('details').open = true;
      $('primary').disabled = true;
      return;
    }
    const data = /** @type {{message?: string; status?: string; overall?: number}} */ (message.data);
    const waiting = message.id ? pending.get(message.id) : undefined;
    if (message.event === 'completed' || message.event === 'failed' || message.event === 'cancelled') {
      if (message.id) pending.delete(message.id);
      if (waiting) {
        if (message.event === 'completed') waiting.resolve(message.data);
        else waiting.reject(new ProductError(data.message ?? copy.cancelled));
      } else if (message.event === 'failed' && !message.id) {
        error(new ProductError(data.message ?? copy.setupFailed));
        $('primary').disabled = true;
      }
    } else if (message.event === 'progress') {
      $('status').textContent = data.status ?? copy.preparingInstallation;
      if (typeof data.overall === 'number' && Number.isFinite(data.overall)) maximum = Math.max(maximum, Math.min(99, data.overall));
      $('total').value = maximum;
    } else if (message.event === 'closing') {
      $('primary').hidden = true; $('back').hidden = true; $('close').disabled = true;
      $('status').textContent = copy.cancelling;
    }
  };
  setInterval(updateWait, 1000);
  $('locale').replaceChildren(...bootstrap.locales.map(locale => new Option(locale.label, locale.id)));
  $('locale').value = bootstrap.locale;
  document.documentElement.lang = bootstrap.locale;
  $('installRoot').value = bootstrap.installRoot;
  show('language');
  send('ready');
})();
