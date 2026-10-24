# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (C) 2026 Artificial Sweetener and contributors
"""Interrupt installer launcher publication and require rescue without opening the app.

Use an isolated qualification workspace, never an everyday installation. The
installer must contain a signed update accepted by that installation's trust.
Artifacts belong on the capture drive. Only retained private-desktop process
handles may be faulted; observer cleanup never counts as application cleanup.
"""
import argparse
import hashlib
import json
from pathlib import Path
import subprocess
import sys
import time
import traceback

REPO = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(REPO))
from tools.splash_parity.offscreen_native import PrivateDesktop
from tools.updater_native_support import run_available


def digest(path):
    """Hash file bytes without reading an entire executable into memory."""
    with path.open('rb') as stream:
        return hashlib.file_digest(stream, 'sha256').hexdigest()


def validate_paths(workspace, installation, setup, output):
    """Constrain fixture mutation to its explicit workspace and reject redirected ancestors."""
    for path in (workspace, installation, setup, output):
        if not path.is_absolute():
            raise ValueError('Qualification paths must be absolute')
        if any(parent.is_symlink() or parent.is_junction() for parent in (path, *path.parents)):
            raise ValueError('Qualification paths cannot traverse redirected storage')
    if installation.resolve() != (workspace / 'installed').resolve():
        raise ValueError('Installation must be the qualification workspace installed directory')
    if output.resolve().is_relative_to(workspace.resolve()):
        raise ValueError('Evidence must remain outside the installation workspace')
    if not setup.is_file():
        raise ValueError('Setup executable is absent')


def settle(owner, seconds, report):
    """Observe natural retirement before any fixture cleanup, retaining descendant evidence."""
    deadline = time.monotonic() + seconds
    while time.monotonic() < deadline:
        owner.discover()
        owner.observe_images(report)
        if not owner.alive():
            return
        time.sleep(0.02)
    raise RuntimeError('Owned process family did not settle naturally')


def require_selection_preserved(before, after):
    """Preparation may advance pending work but cannot replace accepted application authority."""
    if after['accepted'] != before['accepted']:
        raise RuntimeError('Preparation changed accepted application authority')


class Qualification:
    """Own one interruption and repair campaign with exclusive evidence output."""
    def __init__(self, options):
        self.options = options
        self.root, self.work = options.installation, options.workspace
        self.case, self.setup = options.output, options.setup
        self.launcher = self.root / 'sugarsubstitute-launcher.exe'

    def inside(self, phase):
        """Launch the real installer from an ordinary parent on the private desktop."""
        command = [str(self.setup), '--workspace', str(self.work), '--qualify-maintenance',
            str(self.case / f'{phase}-receipt.json'), '--native-installation', str(self.root)]
        with (self.case / f'{phase}-stderr.log').open('wb') as err, \
                (self.case / f'{phase}-stdout.log').open('wb') as out:
            with subprocess.Popen(command, cwd=self.case, stdin=subprocess.DEVNULL, stdout=out,
                    stderr=err, close_fds=True, creationflags=subprocess.CREATE_NO_WINDOW) as process:
                (self.case / f'{phase}-entry.json').write_text(
                    json.dumps({'pid': process.pid, 'command': command}))
                code = process.wait(timeout=360)
                (self.case / f'{phase}-exit.json').write_text(json.dumps({'exit_code': code}))
                if phase == 'rescue' and code:
                    raise RuntimeError(f'Rescue failed: {code}')

    def launch(self, owner, phase):
        """Keep helper and installer descendants in the same retained process family."""
        return owner.launch(Path(sys.executable), REPO, [str(Path(__file__)),
            '--workspace', str(self.work), '--installation', str(self.root),
            '--setup', str(self.setup), '--output', str(self.case), '--inside', phase])

    def interrupt(self, owner, report):
        """Fault the exact installer after its journal and nonempty source exist together."""
        helper = self.launch(owner, 'fault')
        entry = self.case / 'fault-entry.json'
        deadline = time.monotonic() + 300
        faulted = None
        while time.monotonic() < deadline:
            owner.discover()
            owner.observe_images(report)
            if entry.is_file() and faulted is None:
                pid = json.loads(entry.read_text())['pid']
                if pid in owner.handles:
                    faulted = pid
            if faulted is not None:
                for source in self.root.glob('.substitute-publication-*.stage'):
                    try:
                        if source.stat().st_size == 0:
                            continue
                    except FileNotFoundError:
                        continue
                    owner.suspend(faulted)
                    journal = source.with_suffix('.json')
                    if not source.is_file() or not journal.is_file() or self.launcher.exists():
                        raise RuntimeError('Publication completed before suspension; no crash claim')
                    report['journal_before_fault'] = json.loads(journal.read_text())
                    report['staged_bytes_before_fault'] = source.stat().st_size
                    report['faulted_pid'] = faulted
                    report['faulted_creation_filetime'] = owner.creations[faulted]
                    owner.stop(faulted)
                    return source, journal
            if helper not in owner.alive():
                raise RuntimeError('Installer finished without observing an interruptible publication')
            time.sleep(0.002)
        raise RuntimeError('No durable launcher stage observed before deadline')

    def run(self):
        """Preserve original launcher bytes and measure repair before fixture cleanup."""
        validate_paths(self.work, self.root, self.setup, self.case)
        self.case.mkdir()
        backup = self.case / 'verified-original-launcher.exe'
        if not run_available(self.root):
            raise RuntimeError('Qualification installation is already running')
        ownership = json.loads((self.root / '.selection-state/.launcher-generation.json').read_text())
        expected = ownership['sha256']
        if digest(self.launcher) != expected:
            raise RuntimeError('Existing launcher differs from protected ownership')
        if list(self.root.glob('.substitute-publication-*')):
            raise RuntimeError('Fixture requires no pre-existing publication attempts')
        selection = self.root / '.selection-state/.substitute-selection.json'
        payload_root = self.work / 'payloads-v1'
        prior_extractions = set(payload_root.glob('*'))
        report = {'passed': False, 'scope': 'installer_publication_interruption_and_rescue_without_open',
            'expected_launcher_sha256': expected, 'selection_before': json.loads(selection.read_text())}
        owner = PrivateDesktop()
        report['desktop'] = owner.name
        try:
            self.launcher.rename(backup)
            source, journal = self.interrupt(owner, report)
            settle(owner, 60, report)
            report['after_fault_alive'] = owner.alive()
            report['extractions_after_fault'] = len(set(payload_root.glob('*')) - prior_extractions)
            if report['extractions_after_fault'] != 1:
                raise RuntimeError('Installer did not reuse its verified initial extraction')
            if not source.is_file() or not journal.is_file() or self.launcher.exists():
                raise RuntimeError('Expected durable unpublished recovery inputs absent')
            report['staged_sha256_after_process_exit'] = digest(source)
            report['run_available_after_fault'] = run_available(self.root)
            if not report['run_available_after_fault']:
                raise RuntimeError('Installer interruption left an application lock held')
            self.launch(owner, 'rescue')
            settle(owner, 360, report)
            receipt = json.loads((self.case / 'rescue-receipt.json').read_text())
            if not receipt.get('passed') or receipt.get('application_creation_observed') is not False:
                raise RuntimeError('Real installer rescue did not succeed without opening the app')
            if source.exists() or journal.exists() or list(self.root.glob('.substitute-publication-*')):
                raise RuntimeError('Real installer rescue left its interrupted publication inputs')
            if digest(self.launcher) != expected or digest(backup) != expected:
                raise RuntimeError('Rescued launcher differs from original owned bytes')
            report['rescue_receipt'] = receipt
            report['extractions_after_rescue'] = len(set(payload_root.glob('*')) - prior_extractions)
            if report['extractions_after_rescue'] != 2:
                raise RuntimeError('Rescue did not reuse its verified initial extraction')
            report['run_available_after_rescue'] = run_available(self.root)
            if not report['run_available_after_rescue']:
                raise RuntimeError('Rescue left application exclusion held')
            report['selection_after'] = json.loads(selection.read_text())
            require_selection_preserved(report['selection_before'], report['selection_after'])
            owner.observe_images(report)
            if any(Path(path).name.lower() == 'sugarsubstitute.exe'
                    for path in report['process_images'].values()):
                raise RuntimeError('Preparation unexpectedly launched the application')
            report['passed'] = True
        except BaseException as error:
            report['error'] = str(error)
            report['traceback'] = traceback.format_exc()
            raise
        finally:
            try:
                report['process_creation_filetimes'] = owner.creations.copy()
                report['pre_fixture_cleanup_alive'] = owner.alive()
                report['pre_fixture_cleanup_exit_codes'] = owner.exit_codes()
                (self.case / 'result.json').write_text(json.dumps(report, indent=2))
            finally:
                try:
                    owner.close()
                finally:
                    if not report['passed'] and not self.launcher.exists() \
                            and backup.exists() and digest(backup) == expected:
                        backup.rename(self.launcher)
        print(json.dumps(report, indent=2))


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ('workspace', 'installation', 'setup', 'output'):
        parser.add_argument(f'--{name}', type=Path, required=True)
    parser.add_argument('--inside', choices=('fault', 'rescue'), help=argparse.SUPPRESS)
    options = parser.parse_args()
    qualification = Qualification(options)
    if options.inside:
        qualification.inside(options.inside)
    else:
        qualification.run()


if __name__ == '__main__':
    main()
