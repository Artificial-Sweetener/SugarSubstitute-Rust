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
/** Prove exclusive reference discovery, contained credential offers, and modal cancellation. */
import { nextTick } from 'vue';
import { jest } from '@jest/globals';
import { ModelDiscoveryDialog } from '../../../frontend/comfyui/ui/model_library/ModelDiscoveryDialog.js';
import { discoveryPlan, discoveryPorts } from '../support/model_discovery.js';

/** Settle asynchronous service and Vue updates before inspecting user-visible state. */
async function settle(): Promise<void> {
  await new Promise<void>((resolve) => setTimeout(resolve, 0));
  await nextTick();
}

/** Locate a uniquely labelled button on the actual modal surface. */
function button(text: string): HTMLButtonElement {
  const result = [...document.querySelectorAll('button')].find(
    (element) => element.textContent === text,
  );
  if (!result) throw new Error('Missing modal button: ' + text);
  return result;
}

/** Choose the protected alternate through its actual provider context menu. */
async function protectedSelection(): Promise<void> {
  document.querySelector<HTMLButtonElement>('.sc-model-discovery-portrait')?.click();
  await nextTick();
  document
    .querySelector('.sc-model-discovery-card')
    ?.dispatchEvent(new MouseEvent('contextmenu', { bubbles: true, cancelable: true }));
  await nextTick();
  button('Download from CivitAI').click();
  await nextTick();
}

test('presents provider icons and independent checked source rows with keyboard navigation', async () => {
  const ports = discoveryPorts();
  const dialog = new ModelDiscoveryDialog(
    document,
    'upscale_models',
    '',
    ports.discovery,
    ports.credentials,
    jest.fn(),
  );
  const closed = dialog.open();
  try {
    await settle();
    document
      .querySelector('.sc-model-discovery-card')
      ?.dispatchEvent(new MouseEvent('contextmenu', { bubbles: true, cancelable: true }));
    await nextTick();
    const menu = document.querySelector<HTMLElement>('[role=menu]');
    expect(menu?.closest('[role=dialog]')).toBeNull();
    expect(menu?.style.position).toBe('fixed');
    const sources = [...document.querySelectorAll<HTMLButtonElement>('[role=menuitemradio]')];
    expect(sources.map((row) => row.textContent)).toEqual([
      'Download from OpenModelDB',
      'Download from CivitAI',
    ]);
    expect(sources.map((row) => row.getAttribute('aria-checked'))).toEqual(['true', 'false']);
    expect(document.querySelectorAll('.sc-context-menu .sc-model-provider-icon')).toHaveLength(4);
    const first = document.querySelector<HTMLElement>('[role=menuitem]');
    expect(document.activeElement).toBe(first);
    for (let step = 0; step < 3; step += 1) {
      document.activeElement?.dispatchEvent(
        new KeyboardEvent('keydown', { key: 'ArrowDown', bubbles: true, cancelable: true }),
      );
    }
    expect(document.activeElement).toBe(sources[1]);
    sources[1]?.click();
    await nextTick();
    expect(document.querySelector('a.sc-model-discovery-provider')?.getAttribute('href')).toBe(
      'https://civitai.com/models/100',
    );
    expect(document.querySelector('[role=checkbox]')?.getAttribute('aria-checked')).toBe('false');
    expect(ports.discovery.acquire).not.toHaveBeenCalled();
    expect(document.querySelector('[role=menu]')).toBeNull();
  } finally {
    dialog.close();
    await closed;
  }
});

test('opens the selected provider page from its badge and settles a failed preview without losing selection', async () => {
  const ports = discoveryPorts();
  const plan = discoveryPlan();
  const item = plan.suggestions[0];
  if (!item) throw new Error('Missing reviewed fixture suggestion.');
  ports.discovery.discover.mockResolvedValue({
    ...plan,
    suggestions: [
      {
        ...item,
        modelName: 'Reviewed|portrait',
        offers: item.offers.map((offer) => ({ ...offer, previewUrl: '/unavailable-preview' })),
      },
    ],
  });
  const dialog = new ModelDiscoveryDialog(
    document,
    'upscale_models',
    '',
    ports.discovery,
    ports.credentials,
    jest.fn(),
  );
  const closed = dialog.open();
  try {
    await settle();
    const badge = document.querySelector<HTMLAnchorElement>('a.sc-model-discovery-provider');
    expect(document.querySelector('.sc-model-discovery-caption')?.textContent).toBe(
      'Reviewed|portrait',
    );
    expect(document.querySelector('.sc-model-discovery-caption wbr')).not.toBeNull();
    expect(badge?.getAttribute('href')).toBe('https://openmodeldb.info/models/portrait');
    expect(badge?.getAttribute('target')).toBe('_blank');
    expect(badge?.getAttribute('rel')).toBe('noopener noreferrer');
    expect(document.querySelector('.sc-model-preview-busy-ring')).not.toBeNull();
    document.querySelector('.sc-model-discovery-portrait img')?.dispatchEvent(new Event('error'));
    await nextTick();
    expect(document.querySelector('.sc-model-discovery-portrait img')).toBeNull();
    expect(document.querySelector('.sc-model-preview-unavailable')?.textContent).toBe(
      'Preview unavailable',
    );
    expect(
      document.querySelector('.sc-model-discovery-portrait .sc-model-preview-busy-ring'),
    ).toBeNull();
    await protectedSelection();
    expect(
      document.querySelector('[role=img][aria-label="CivitAI API key required"]'),
    ).not.toBeNull();
    expect(document.querySelector('a.sc-model-discovery-provider')?.getAttribute('href')).toBe(
      'https://civitai.com/models/100',
    );
    expect(document.querySelector('[role=checkbox]')?.getAttribute('aria-checked')).toBe('true');
    expect(button('Add CivitAI key').disabled).toBe(false);
    expect(ports.discovery.acquire).not.toHaveBeenCalled();
  } finally {
    dialog.close();
    await closed;
  }
});

test('uses the reference Fluent action text size in discovery and its credential layer', async () => {
  const ports = discoveryPorts();
  const dialog = new ModelDiscoveryDialog(
    document,
    'upscale_models',
    '',
    ports.discovery,
    ports.credentials,
    jest.fn(),
  );
  const closed = dialog.open();
  try {
    await settle();
    expect(getComputedStyle(button('Not now')).fontSize).toBe('14px');
    expect(getComputedStyle(button('Download and use')).fontSize).toBe('14px');
    await protectedSelection();
    button('Add CivitAI key').click();
    await nextTick();
    expect(getComputedStyle(button('Cancel')).fontSize).toBe('14px');
    expect(getComputedStyle(button('Save and continue')).fontSize).toBe('14px');
  } finally {
    dialog.close();
    await closed;
  }
});

test('renders literal cards, starts unselected, and changes only the deliberately acquired field', async () => {
  const ports = discoveryPorts();
  const installed = jest.fn();
  const dialog = new ModelDiscoveryDialog(
    document,
    'upscale_models',
    '',
    ports.discovery,
    ports.credentials,
    installed,
  );
  const closed = dialog.open();
  try {
    await settle();
    expect(document.querySelector('[role=dialog]')?.getAttribute('aria-label')).toBe(
      'Download an upscaler model?',
    );
    expect(document.querySelector('.sc-model-discovery-caption')?.textContent).toBe(
      '<b>Reviewed portrait</b>',
    );
    expect(document.querySelector('.sc-model-discovery-caption b')).toBeNull();
    expect(document.querySelector('[role=checkbox]')?.getAttribute('aria-checked')).toBe('false');
    expect(button('Download and use').disabled).toBe(true);
    expect(ports.discovery.acquire).not.toHaveBeenCalled();
    document.querySelector<HTMLButtonElement>('[role=checkbox]')?.click();
    await nextTick();
    button('Download and use').click();
    await settle();
    expect(ports.discovery.acquire.mock.calls[0]?.slice(0, 3)).toEqual([
      'a'.repeat(32),
      discoveryPlan().suggestions[0]?.identity,
      'openmodeldb',
    ]);
    expect(installed).toHaveBeenCalledWith('portrait.pth');
    await closed;
    expect(ports.discovery.release).toHaveBeenCalledWith('a'.repeat(32));
    expect(document.querySelector('[role=dialog]')).toBeNull();
  } finally {
    dialog.close();
    await closed;
  }
});

test('keeps provider-source selection exact and saves an explicit key before resuming its protected offer', async () => {
  const ports = discoveryPorts();
  const dialog = new ModelDiscoveryDialog(
    document,
    'upscale_models',
    '',
    ports.discovery,
    ports.credentials,
    jest.fn(),
  );
  const closed = dialog.open();
  try {
    await settle();
    await protectedSelection();
    button('Add CivitAI key').click();
    await nextTick();
    expect(ports.discovery.acquire).not.toHaveBeenCalled();
    const input = document.querySelector<HTMLInputElement>('.sc-model-credential-input input');
    if (!input) throw new Error('Missing contained credential input.');
    expect(input.type).toBe('password');
    expect(button('Save and continue').disabled).toBe(true);
    input.value = 'fixture-only-key';
    input.dispatchEvent(new Event('input', { bubbles: true }));
    await nextTick();
    button('Save and continue').click();
    await settle();
    expect(ports.credentials.save.mock.calls[0]?.[0]).toBe('fixture-only-key');
    expect(ports.discovery.acquire.mock.calls[0]?.[2]).toBe('civitai');
    await closed;
    expect(document.body.textContent).not.toContain('fixture-only-key');
    expect(localStorage.length).toBe(0);
  } finally {
    dialog.close();
    await closed;
  }
});

test('public-only discovery replaces and releases the reviewed plan without keeping its old selection', async () => {
  const ports = discoveryPorts();
  const dialog = new ModelDiscoveryDialog(
    document,
    'upscale_models',
    '',
    ports.discovery,
    ports.credentials,
    jest.fn(),
  );
  const closed = dialog.open();
  try {
    await settle();
    await protectedSelection();
    button('Add CivitAI key').click();
    await nextTick();
    button("Show models that don't require a key").click();
    await settle();
    expect(ports.discovery.discover.mock.calls.at(-1)?.slice(0, 3)).toEqual([
      'upscale_models',
      '',
      true,
    ]);
    expect(ports.discovery.release).toHaveBeenCalledTimes(1);
    expect(button('Download and use').disabled).toBe(true);
    expect(button('Show all models').hidden).toBe(false);
    expect(document.querySelector('.sc-model-credential-panel')).toBeNull();
    expect(ports.credentials.save).not.toHaveBeenCalled();
    expect(ports.discovery.acquire).not.toHaveBeenCalled();
  } finally {
    dialog.close();
    await closed;
  }
});

test.each(['retry', 'public source'] as const)(
  'keeps the selected offer after credential storage fails and supports %s',
  async (recovery) => {
    const ports = discoveryPorts();
    ports.credentials.save.mockRejectedValueOnce(
      new Error('The CivitAI credential could not be saved.'),
    );
    const report = jest.spyOn(console, 'error').mockImplementation(() => undefined);
    const selected = jest.fn();
    const dialog = new ModelDiscoveryDialog(
      document,
      'upscale_models',
      '',
      ports.discovery,
      ports.credentials,
      selected,
    );
    const closed = dialog.open();
    try {
      await settle();
      await protectedSelection();
      button('Add CivitAI key').click();
      await nextTick();
      const input = document.querySelector<HTMLInputElement>('.sc-model-credential-input input');
      if (!input) throw new Error('Missing protected-source credential input.');
      input.value = 'fictional-storage-failure-proof';
      input.dispatchEvent(new Event('input', { bubbles: true }));
      await nextTick();
      button('Save and continue').click();
      await settle();
      expect(document.querySelector('.sc-model-credential-panel')?.textContent).toContain(
        'The CivitAI credential could not be saved.',
      );
      expect(input.value).toBe('');
      expect(button('Save and continue').disabled).toBe(true);
      expect(document.querySelector('[role=checkbox]')?.getAttribute('aria-checked')).toBe('true');
      expect(ports.discovery.acquire).not.toHaveBeenCalled();
      expect(selected).not.toHaveBeenCalled();
      expect(document.body.textContent).not.toContain('fictional-storage-failure-proof');
      expect(localStorage.length).toBe(0);
      if (recovery === 'retry') {
        input.value = 'fictional-storage-failure-proof';
        input.dispatchEvent(new Event('input', { bubbles: true }));
        await nextTick();
        expect(button('Save and continue').disabled).toBe(false);
        button('Save and continue').click();
      } else {
        document
          .querySelector('.sc-model-credential-panel')
          ?.dispatchEvent(
            new KeyboardEvent('keydown', { key: 'Escape', bubbles: true, cancelable: true }),
          );
        await nextTick();
        document
          .querySelector('.sc-model-discovery-card')
          ?.dispatchEvent(new MouseEvent('contextmenu', { bubbles: true, cancelable: true }));
        await nextTick();
        button('Download from OpenModelDB').click();
        await nextTick();
        expect(document.querySelector('[role=checkbox]')?.getAttribute('aria-checked')).toBe(
          'true',
        );
        button('Download and use').click();
      }
      await settle();
      await closed;
      expect(ports.discovery.acquire).toHaveBeenCalledTimes(1);
      expect(ports.discovery.acquire.mock.calls[0]?.[2]).toBe(
        recovery === 'retry' ? 'civitai' : 'openmodeldb',
      );
      expect(ports.credentials.save).toHaveBeenCalledTimes(recovery === 'retry' ? 2 : 1);
      expect(selected).toHaveBeenCalledTimes(1);
    } finally {
      dialog.close();
      await closed;
      report.mockRestore();
    }
  },
);

test('contains offline failures and aborts live planning when its originating interaction closes', async () => {
  const ports = discoveryPorts();
  const logging = jest.spyOn(console, 'error').mockImplementation(() => undefined);
  ports.discovery.discover.mockRejectedValueOnce(new Error('<b>Provider offline</b>'));
  const failed = new ModelDiscoveryDialog(
    document,
    'upscale_models',
    '',
    ports.discovery,
    ports.credentials,
    jest.fn(),
  );
  const failedClosed = failed.open();
  try {
    await settle();
    expect(document.querySelector('.sc-model-discovery-status')?.textContent).toBe(
      '<b>Provider offline</b>',
    );
    expect(document.querySelector('.sc-model-discovery-status b')).toBeNull();
    expect(document.querySelectorAll('[role=dialog]')).toHaveLength(1);
    expect(document.body.textContent).not.toContain('Model picker unavailable');
    failed.close();
    await failedClosed;
    let signal: AbortSignal | undefined;
    ports.discovery.discover.mockImplementation(
      async (_kind, _target, _public, _progress, current) => {
        signal = current;
        await new Promise<void>((_resolve, reject) =>
          current.addEventListener('abort', () => reject(new Error('Cancelled')), { once: true }),
        );
        return discoveryPlan();
      },
    );
    const pending = new ModelDiscoveryDialog(
      document,
      'upscale_models',
      '',
      ports.discovery,
      ports.credentials,
      jest.fn(),
    );
    const pendingClosed = pending.open();
    await settle();
    pending.close();
    await pendingClosed;
    await settle();
    expect(signal?.aborted).toBe(true);
    expect(document.querySelector('[role=dialog]')).toBeNull();
  } finally {
    failed.close();
    await failedClosed;
    logging.mockRestore();
  }
});

test('keeps usable public offers when credential-status inspection is unavailable', async () => {
  const ports = discoveryPorts();
  const logging = jest.spyOn(console, 'error').mockImplementation(() => undefined);
  ports.credentials.status.mockRejectedValueOnce(new Error('Secure store offline'));
  const dialog = new ModelDiscoveryDialog(
    document,
    'upscale_models',
    '',
    ports.discovery,
    ports.credentials,
    jest.fn(),
  );
  const closed = dialog.open();
  try {
    await settle();
    expect(document.querySelectorAll('[role=checkbox]')).toHaveLength(1);
    expect(button('Download and use').disabled).toBe(true);
  } finally {
    dialog.close();
    await closed;
    logging.mockRestore();
  }
  expect(ports.discovery.release).toHaveBeenCalledWith('a'.repeat(32));
});
