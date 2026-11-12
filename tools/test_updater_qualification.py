# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (C) 2026 Artificial Sweetener and contributors
"""Reject unsafe fault-test targets and verify read-only native exclusion observations."""
import ctypes
import hashlib
import json
from pathlib import Path
import tempfile
from types import SimpleNamespace
from copy import deepcopy
import unittest
from unittest.mock import Mock, patch

from tools import updater_native_support as native
from tools.updater_publication_qualification import (
    Qualification, settle, validate_paths, require_selection_preserved,
    payload_cache_root, payload_generations, require_payload_reuse,
)


class InvocationRetirement(unittest.TestCase):
    def test_recovered_preparation_must_preserve_accepted_and_exact_candidate(self):
        generation = {'id': 'prepared', 'package': 'authenticated'}
        accepted = {'id': 'running', 'package': 'original'}
        report = {'accepted_with_main': True, 'prepared_while_running': generation,
            'selection_before_close': {'accepted': accepted},
            'selection': {'accepted': accepted, 'candidate': {'phase': 'prepared',
                'generation': generation}}}
        native.require_prepared_while_running(report)
        for candidate in (None, {'phase': 'staging', 'generation': generation},
                          {'phase': 'prepared', 'generation': accepted}):
            changed = deepcopy(report)
            changed['selection']['candidate'] = candidate
            with self.assertRaises(RuntimeError):
                native.require_prepared_while_running(changed)
        changed = deepcopy(report)
        changed['selection']['accepted'] = generation
        with self.assertRaises(RuntimeError):
            native.require_prepared_while_running(changed)

    def test_immutable_frames_publish_without_replacing_a_readers_open_file(self):
        with tempfile.TemporaryDirectory() as temporary:
            destination = Path(temporary) / 'entry.json'
            self.assertIsNone(native.latest_family_receipt(destination))
            frames = destination.with_suffix('.frames')
            frames.mkdir()
            first = frames / '00000001.json'
            native.publish_receipt(first, {'entry_pid': 22})
            with first.open('rb') as held:
                native.publish_receipt(frames / '00000002.json', {'entry_pid': 23})
                self.assertEqual(native.latest_family_receipt(destination), {'entry_pid': 23})
                self.assertEqual(json.load(held), {'entry_pid': 22})
            with self.assertRaisesRegex(RuntimeError, 'immutable'):
                native.publish_receipt(first, {'entry_pid': 99})

    def test_completed_original_family_is_required(self):
        good = {'completed': True, 'pre_fixture_cleanup_alive': [], 'entry_pid': 22}
        native.require_invocation(good, 22)
        for changes in ({'completed': False}, {'error': 'Observation failed'},
                        {'pre_fixture_cleanup_alive': [23]}, {'entry_pid': 24}):
            with self.subTest(changes=changes), self.assertRaises(RuntimeError):
                native.require_invocation(dict(good, **changes), 22)
        with self.assertRaises(RuntimeError):
            native.require_invocation({}, 22)


class NativeSelectionContention(unittest.TestCase):
    def test_existing_writer_lock_uses_whole_range_and_releases_exact_handle_once(self):
        create, close, lock = Mock(return_value=123), Mock(return_value=True), Mock(return_value=True)
        with patch.object(native.ctypes, 'WinDLL'), \
                patch.object(native, 'declare', side_effect=[create, close, lock]):
            writer = native.SelectionWriter(Path('X:/fixture/installed'))
            writer.close()
            writer.close()
        self.assertEqual(create.call_args.args[1:6], (0xC0000000, 3, None, 3, 0x200000))
        self.assertEqual(Path(create.call_args.args[0]).name, '.selection-lock')
        self.assertEqual(lock.call_args.args[:5], (123, 3, 0, 0xffffffff, 0xffffffff))
        close.assert_called_once_with(123)

    def test_failed_lock_acquisition_closes_only_its_own_handle(self):
        create, close, lock = Mock(return_value=456), Mock(return_value=True), Mock(return_value=False)
        with patch.object(native.ctypes, 'WinDLL'), \
                patch.object(native, 'declare', side_effect=[create, close, lock]), \
                patch.object(native.ctypes, 'get_last_error', return_value=33):
            with self.assertRaises(OSError):
                native.SelectionWriter(Path('X:/fixture/installed'))
        close.assert_called_once_with(456)


class InstallerFinishObservation(unittest.TestCase):
    def setUp(self):
        self.report = {
            'receipt': {'passed': True, 'application_creation_observed': False},
            'selection_before': None,
            'selection': {'accepted': None, 'candidate': {'phase': 'prepared',
                'generation': {'id': 'prepared', 'package': 'authenticated'}}},
            'splash_hwnds': [],
        }

    def test_first_finish_retains_preparation_without_claiming_health(self):
        original = deepcopy(self.report)
        native.require_installer_finish(self.report)
        self.assertEqual(self.report, original)

    def test_maintenance_finish_preserves_exact_prior_accepted_identity(self):
        accepted = {'id': 'previous', 'package': 'original'}
        self.report['selection_before'] = {'accepted': accepted}
        self.report['selection']['accepted'] = deepcopy(accepted)
        native.require_installer_finish(self.report)
        for altered in (None, {'id': 'changed', 'package': 'original'},
                        {'id': 'previous', 'package': 'changed'}):
            with self.subTest(accepted=altered):
                self.report['selection']['accepted'] = altered
                with self.assertRaisesRegex(RuntimeError, 'preserve accepted state'):
                    native.require_installer_finish(self.report)

    def test_missing_or_unfinished_preparation_cannot_pass_finish(self):
        for state in (None, {}, {'accepted': None, 'candidate': None},
                      {'accepted': None, 'candidate': {'phase': 'staging'}},
                      {'accepted': None, 'candidate': {'phase': 'trial'}}):
            with self.subTest(state=state):
                self.report['selection'] = state
                with self.assertRaisesRegex(RuntimeError, 'preserve accepted state'):
                    native.require_installer_finish(self.report)

    def test_incomplete_receipt_or_any_application_observation_rejects_finish(self):
        for changes in ({'receipt': {}}, {'receipt': {'passed': True}},
                        {'receipt': {'passed': False, 'application_creation_observed': False}},
                        {'receipt': {'passed': True, 'application_creation_observed': True}},
                        {'application_root_pid': 10}, {'splash_hwnds': [12]}):
            with self.subTest(changes=changes):
                report = {**self.report, **changes}
                with self.assertRaisesRegex(RuntimeError, 'without application creation'):
                    native.require_installer_finish(report)

    def test_prepared_phase_without_an_exact_generation_is_not_completion(self):
        for generation in (None, {}, {'id': 'prepared'}, {'id': '', 'package': 'authenticated'},
                           {'id': 'prepared', 'package': 42}):
            with self.subTest(generation=generation):
                self.report['selection']['candidate']['generation'] = generation
                with self.assertRaisesRegex(RuntimeError, 'identify its prepared generation'):
                    native.require_installer_finish(self.report)


class PublicationBoundaries(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.base = Path(self.temporary.name)
        self.work = self.base / 'workspace'
        self.work.mkdir()
        self.root = self.work / 'installed'
        self.root.mkdir()
        self.setup = self.base / 'setup.exe'
        self.setup.write_bytes(b'inert fixture')
        self.output = self.base / 'evidence'

    def test_payload_namespace_comes_from_the_registered_catalog(self):
        catalog = self.base / 'catalog.toml'
        contents = ('schema_version = 1\n'
            '[[caches]]\nid = "runtime-cache-storage"\nnamespace = "owned-root-v2"\n'
            '[[caches]]\nid = "setup-packed-payloads"\nnamespace = "payload-content-v3"\n')
        catalog.write_text(contents, encoding='utf-8')
        self.assertEqual(payload_cache_root(self.work, catalog),
            self.work / 'owned-root-v2' / 'payload-content-v3')
        for invalid in (
                contents.replace('schema_version = 1', 'schema_version = 99'),
                contents.replace('schema_version = 1', 'schema_version = true'),
                contents.replace('payload-content-v3', '../foreign'),
                contents.replace('payload-content-v3', 'owned-root-v2'),
                contents + '[[caches]]\nid = "setup-packed-payloads"\nnamespace = "other"\n'):
            catalog.write_text(invalid, encoding='utf-8')
            with self.assertRaises(ValueError):
                payload_cache_root(self.work, catalog)

    def test_shared_payload_inventory_preserves_unrelated_and_legacy_content(self):
        legacy = self.work / 'payloads-v1'
        legacy.mkdir()
        (legacy / 'authored').write_bytes(b'preserve')
        root = payload_cache_root(self.work)
        self.assertEqual(payload_generations(root), {})
        root.mkdir(parents=True)
        unrelated = root / 'unknown-file'
        unrelated.write_bytes(b'unknown bytes')
        generation = root / ('a' * 64 + '-0000000000000001')
        generation.mkdir()
        first = payload_generations(root)
        self.assertEqual(require_payload_reuse({}, first, payload_generations(root)), 1)
        self.assertEqual(set(first), {generation.name})
        self.assertEqual(unrelated.read_bytes(), b'unknown bytes')
        self.assertEqual((legacy / 'authored').read_bytes(), b'preserve')
        with patch.object(Path, 'is_junction', lambda path: path == generation):
            with self.assertRaisesRegex(ValueError, 'redirected'):
                payload_generations(root)
        with patch.object(Path, 'is_junction', lambda path: path == root.parent):
            with self.assertRaisesRegex(ValueError, 'redirected'):
                payload_generations(root)
        actual_stat = Path.stat

        def absent_identity(path, **options):
            value = actual_stat(path, **options)
            return SimpleNamespace(st_mode=value.st_mode, st_dev=value.st_dev, st_ino=0) \
                if path == generation else value

        with patch.object(Path, 'stat', absent_identity):
            with self.assertRaisesRegex(RuntimeError, 'no observable file identity'):
                payload_generations(root)

    def test_rescue_rejects_empty_duplicate_or_replaced_generation_evidence(self):
        first = {'original': (1, 2)}
        self.assertEqual(require_payload_reuse(first, first, first), 0)
        for before, initial, rescued in (
                ({}, {}, {}),
                ({}, {**first, 'duplicate': (1, 3)}, first),
                (first, {'original': (1, 3)}, {'original': (1, 3)}),
                ({}, first, {**first, 'duplicate': (1, 3)}),
                ({}, first, {'original': (1, 3)}),
                ({}, first, {})):
            with self.subTest(before=before, initial=initial, rescued=rescued):
                with self.assertRaises(RuntimeError):
                    require_payload_reuse(before, initial, rescued)

    def test_explicit_isolated_workspace_accepts_external_evidence(self):
        validate_paths(self.work, self.root, self.setup, self.output)
        self.assertFalse(self.output.exists())

    def test_foreign_installation_cannot_be_mutated(self):
        with self.assertRaisesRegex(ValueError, 'workspace installed'):
            validate_paths(self.work, self.base / 'everyday-installation', self.setup, self.output)

    def test_evidence_cannot_replace_installation_workspace_contents(self):
        with self.assertRaisesRegex(ValueError, 'Evidence must remain outside'):
            validate_paths(self.work, self.root, self.setup, self.root / 'evidence')

    def test_redirected_ancestor_is_rejected_before_resolving_mutation_targets(self):
        with patch.object(Path, 'is_junction', lambda path: path == self.work):
            with self.assertRaisesRegex(ValueError, 'redirected storage'):
                validate_paths(self.work, self.root, self.setup, self.output)

    def test_relative_paths_and_absent_setup_are_rejected(self):
        with self.assertRaisesRegex(ValueError, 'absolute'):
            validate_paths(self.work, self.root, self.setup, Path('relative'))
        self.setup.unlink()
        with self.assertRaisesRegex(ValueError, 'Setup executable is absent'):
            validate_paths(self.work, self.root, self.setup, self.output)

    def test_preparation_can_change_pending_work_without_claiming_health(self):
        require_selection_preserved(
            {'accepted': {'id': 'old', 'package': 'hash'}, 'candidate': None},
            {'accepted': {'id': 'old', 'package': 'hash'}, 'candidate': {'id': 'pending'}})

    def test_repair_cannot_change_any_accepted_identity_field(self):
        before = {'accepted': {'id': 'old', 'package': 'hash'}}
        for accepted in (None, {'id': 'new', 'package': 'hash'}, {'id': 'old', 'package': 'changed'}):
            with self.subTest(accepted=accepted):
                with self.assertRaisesRegex(RuntimeError, 'accepted application authority'):
                    require_selection_preserved(before, {'accepted': accepted})

    def failed_attempt(self, replace_launcher=False):
        state = self.root / '.selection-state'
        state.mkdir()
        original = b'verified launcher'
        launcher = self.root / 'sugarsubstitute-launcher.exe'
        launcher.write_bytes(original)
        (state / '.launcher-generation.json').write_text(
            json.dumps({'sha256': hashlib.sha256(original).hexdigest()}))
        (state / '.substitute-selection.json').write_text(json.dumps({'accepted': {'id': 'old'}}))
        qualification = Qualification(SimpleNamespace(workspace=self.work, installation=self.root,
            setup=self.setup, output=self.output))
        owner = Mock(name='retained fixture owner')
        owner.name, owner.creations = 'private fixture', {10: 12345}
        owner.alive.return_value = [10]
        owner.exit_codes.return_value = {'10': 259}

        def interrupt(*_):
            if replace_launcher:
                launcher.write_bytes(b'unknown replacement')
            raise RuntimeError('Injected observation failure')

        with patch('tools.updater_publication_qualification.PrivateDesktop', return_value=owner), \
                patch('tools.updater_publication_qualification.run_available', return_value=True), \
                patch.object(qualification, 'interrupt', side_effect=interrupt):
            with self.assertRaisesRegex(RuntimeError, 'Injected observation failure'):
                qualification.run()
        owner.close.assert_called_once_with()
        report = json.loads((self.output / 'result.json').read_text())
        self.assertFalse(report['passed'])
        self.assertEqual(report['process_creation_filetimes'], {'10': 12345})
        self.assertEqual(report['pre_fixture_cleanup_alive'], [10])
        return launcher, self.output / 'verified-original-launcher.exe', original

    def test_failed_observer_restores_missing_launcher_and_retains_process_birth_evidence(self):
        launcher, backup, original = self.failed_attempt()
        self.assertEqual(launcher.read_bytes(), original)
        self.assertFalse(backup.exists())

    def test_failed_observer_never_overwrites_a_new_unknown_launcher(self):
        launcher, backup, original = self.failed_attempt(replace_launcher=True)
        self.assertEqual(launcher.read_bytes(), b'unknown replacement')
        self.assertEqual(backup.read_bytes(), original)


class FamilySettlement(unittest.TestCase):
    """Settlement fails only on a family that stops changing, never on elapsed time alone."""

    def owner(self, families):
        owner = Mock(name='retained fixture owner')
        owner.alive.side_effect = families
        return owner

    def test_a_slowly_retiring_family_settles_however_long_it_takes(self):
        now = [0.0]
        families = [[10, 11, 12], [10, 11], [10], []]

        def pause(_):
            now[0] += 500

        settle(self.owner(families), {}, stall=600, clock=lambda: now[0], pause=pause)
        self.assertEqual(now[0], 1500)

    def test_an_unchanged_family_fails_after_the_whole_stall(self):
        now = [0.0]

        def pause(_):
            now[0] += 100

        with self.assertRaisesRegex(RuntimeError, r'\[10\] made no progress for 600 seconds'):
            settle(self.owner([[10]] * 20), {}, stall=600, clock=lambda: now[0], pause=pause)
        self.assertEqual(now[0], 600)

class NativeRunObservation(unittest.TestCase):
    def probe(self, handle, error=0, allow_missing=False, close_success=True):
        create, close = Mock(return_value=handle), Mock(return_value=close_success)
        with patch.object(native.ctypes, 'WinDLL'), \
                patch.object(native, 'declare', side_effect=[create, close]), \
                patch.object(native.ctypes, 'get_last_error', return_value=error):
            result = native.run_available(Path('X:/fixture/installed'), allow_missing)
        return result, create, close

    def test_probe_opens_existing_shared_file_then_closes_observer_handle(self):
        available, create, close = self.probe(123)
        self.assertTrue(available)
        self.assertEqual(create.call_args.args[1:], (0xC0000000, 7, None, 3, 0x200000, None))
        close.assert_called_once_with(123)

    def test_sharing_conflict_is_busy_and_never_closes_an_invalid_handle(self):
        available, _, close = self.probe(ctypes.c_void_p(-1).value, error=32)
        self.assertFalse(available)
        close.assert_not_called()

    def test_missing_state_is_available_only_for_explicit_fresh_installation(self):
        for error in (2, 3):
            with self.subTest(error=error):
                available, _, close = self.probe(ctypes.c_void_p(-1).value, error, True)
                self.assertTrue(available)
                close.assert_not_called()
                with self.assertRaises(OSError):
                    self.probe(ctypes.c_void_p(-1).value, error)

    def test_access_denied_and_close_failure_are_not_reported_as_available(self):
        with self.assertRaises(OSError):
            self.probe(ctypes.c_void_p(-1).value, error=5, allow_missing=True)
        with self.assertRaises(OSError):
            self.probe(123, error=6, close_success=False)


class ShortcutLaunchParameters(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name) / 'Substitute Installation'
        self.root.mkdir()
        self.target = self.root / 'sugarsubstitute-launcher.exe'
        self.target.write_bytes(b'inert fixture')
        self.shortcut = SimpleNamespace(TargetPath=str(self.target), Arguments='', WorkingDirectory=str(self.root))

    def test_shell_working_directory_is_returned_for_process_creation(self):
        target, directory = native.shortcut_launch(self.shortcut, self.root)
        self.assertEqual(target, self.target)
        self.assertEqual(directory, self.root)

    def test_foreign_targets_arguments_and_working_directories_are_rejected(self):
        for field, value in [('TargetPath', str(self.root / 'foreign.exe')),
                             ('Arguments', '--installation-root elsewhere'),
                             ('WorkingDirectory', str(self.root.parent)),
                             ('WorkingDirectory', ''), ('TargetPath', self.target.name)]:
            with self.subTest(field=field, value=value):
                changed = SimpleNamespace(**{**vars(self.shortcut), field: value})
                with self.assertRaisesRegex(RuntimeError, 'differ from the installed entry'):
                    native.shortcut_launch(changed, self.root)


class StartupFailureObservation(unittest.TestCase):
    def setUp(self):
        self.owner = Mock()
        self.window = dict(hwnd=12, splash=True, visible=True, retry=True, console=False)
        self.report = {}

    def observe(self):
        return native.capture_startup_failure(self.owner, self.window, self.report)

    def test_normal_loading_and_unowned_surface_shapes_are_not_operated(self):
        for field in ('splash', 'visible', 'retry'):
            with self.subTest(field=field):
                window = dict(self.window, **{field: False})
                self.assertFalse(native.capture_startup_failure(self.owner, window, self.report))
                self.assertEqual(self.report, {})
                self.assertEqual(self.owner.mock_calls, [])

    def test_disclosure_is_requested_once_then_diagnostics_precede_normal_close(self):
        self.assertFalse(self.observe())
        self.assertFalse(self.observe())
        self.owner.click_control.assert_called_once_with(12, 1)
        self.owner.close_window.assert_not_called()
        self.window['console'] = True
        self.owner.console_tail.return_value = 'backend failed'
        self.assertTrue(self.observe())
        self.assertEqual(self.report['startup_failures']['12']['console_tail'], 'backend failed')
        self.owner.close_window.assert_called_once_with(12)
        self.assertTrue(self.observe())
        self.owner.console_tail.assert_called_once_with(12)
        self.owner.close_window.assert_called_once_with(12)

    def test_already_open_console_is_read_without_toggling_it_closed(self):
        self.window['console'] = True
        self.owner.console_tail.return_value = ''
        self.assertTrue(self.observe())
        self.owner.click_control.assert_not_called()
        self.assertEqual(self.report['startup_failures']['12']['console_tail'], '')

    def test_disappearing_console_does_not_discard_diagnostics_or_close_early(self):
        self.window['console'] = True
        self.owner.console_tail.return_value = None
        self.assertFalse(self.observe())
        self.owner.close_window.assert_not_called()
        self.assertNotIn('console_tail', self.report['startup_failures']['12'])

    def test_failed_ownership_validation_propagates_without_claiming_closure(self):
        self.owner.click_control.side_effect = RuntimeError('Window is not owned')
        with self.assertRaisesRegex(RuntimeError, 'not owned'):
            self.observe()
        self.owner.close_window.assert_not_called()
        self.assertFalse(self.report['startup_failures']['12']['close_requested'])


class PostExitObservation(unittest.TestCase):
    def setUp(self):
        accepted = {'id': 'original', 'package': 'original-package'}
        self.report = {'download_observed_before_close': True,
            'post_exit': {'root_pid': 17, 'alive': [17, 18], 'run_available': True,
                'backend_released': True, 'visible_application_windows': 0},
            'selection_before_close': {'accepted': accepted},
            'selection': {'accepted': accepted, 'candidate': {'phase': 'prepared'}}}

    def test_preparation_after_observed_application_retirement_passes(self):
        native.require_post_exit(self.report)

    def test_running_application_or_absent_worker_cannot_pass(self):
        for change in ({'alive': []}, {'run_available': False}, {'backend_released': False},
                       {'visible_application_windows': 1}, {'root_pid': None}):
            report = deepcopy(self.report)
            report['post_exit'].update(change)
            with self.subTest(change=change), self.assertRaisesRegex(RuntimeError, 'custody'):
                native.require_post_exit(report)

    def test_missing_or_activated_preparation_cannot_pass(self):
        for change in ({'candidate': None}, {'candidate': {'phase': 'trial'}}, {'accepted': None}):
            report = deepcopy(self.report)
            report['selection'].update(change)
            with self.subTest(change=change), self.assertRaisesRegex(RuntimeError, 'prepare an update'):
                native.require_post_exit(report)


class DuplicateTrialObservation(unittest.TestCase):
    def setUp(self):
        self.generation = {'id': 'first', 'package': 'signed-package'}
        self.report = {'splash_hwnds': [12], 'main_hwnds': [14], 'accepted_with_main': True,
            'selection_at_duplicate': {'candidate': {'phase': 'trial',
                'attempt': {'generation': self.generation}}},
            'duplicate_entry_exit': {'exit_code': 0},
            'duplicate_application_exit_codes': {'42': 0},
            'duplicate_entry_pid': 40,
            'duplicate_family': {'completed': True, 'entry_pid': 40, 'pre_fixture_cleanup_alive': [],
                'process_images': {'42': 'sugarsubstitute.exe'}, 'exit_codes': {'42': 0}},
            'selection': {'accepted': self.generation}}

    def test_trial_requires_visible_splash_before_duplicate_admission(self):
        current = self.report['selection_at_duplicate']
        self.assertTrue(native.duplicate_trial_ready(current, [12]))
        self.assertFalse(native.duplicate_trial_ready(current, []))
        for state in (None, {}, {'candidate': None}, {'candidate': {'phase': 'prepared'}}):
            with self.subTest(state=state):
                self.assertFalse(native.duplicate_trial_ready(state, [12]))

    def test_original_trial_health_and_successful_duplicate_retirement_pass(self):
        native.require_duplicate_trial(self.report)

    def test_duplicate_failure_or_missing_exit_cannot_pass(self):
        for result in (None, {}, {'exit_code': 1}):
            with self.subTest(result=result):
                self.report['duplicate_entry_exit'] = result
                with self.assertRaisesRegex(RuntimeError, 'retire successfully'):
                    native.require_duplicate_trial(self.report)

    def test_launcher_exit_alone_or_live_or_failed_application_cannot_pass(self):
        for codes in (None, {}, {'42': 259}, {'42': 1}):
            with self.subTest(codes=codes):
                self.report['duplicate_application_exit_codes'] = codes
                self.report['duplicate_family']['exit_codes'] = codes or {}
                with self.assertRaises(RuntimeError):
                    native.require_duplicate_trial(self.report)

    def test_wrong_creator_or_forced_cleanup_cannot_pass(self):
        for change in ({'entry_pid': 99}, {'completed': False}, {'pre_fixture_cleanup_alive': [42]},
                       {'error': 'Lost observation'}, {'process_images': {}}):
            report = deepcopy(self.report)
            report['duplicate_family'].update(change)
            with self.subTest(change=change), self.assertRaisesRegex(RuntimeError, 'native exit evidence'):
                native.require_duplicate_trial(report)

    def test_duplicate_windows_cannot_pass_even_when_the_original_becomes_healthy(self):
        for field in ('splash_hwnds', 'main_hwnds'):
            report = dict(self.report, **{field: [12, 14]})
            with self.subTest(field=field):
                with self.assertRaisesRegex(RuntimeError, 'one splash and one main window'):
                    native.require_duplicate_trial(report)

    def test_fallback_to_old_generation_or_missing_health_cannot_pass(self):
        for accepted in (None, {}, {'id': 'old', 'package': 'signed-package'},
                         {'id': 'first', 'package': 'other-package'}):
            with self.subTest(accepted=accepted):
                self.report['selection']['accepted'] = accepted
                with self.assertRaisesRegex(RuntimeError, 'original trial'):
                    native.require_duplicate_trial(self.report)
        self.report['selection']['accepted'] = self.generation
        self.report['accepted_with_main'] = False
        with self.assertRaisesRegex(RuntimeError, 'original trial'):
            native.require_duplicate_trial(self.report)

    def test_missing_trial_or_visible_splash_cannot_pass(self):
        for field in ('selection_at_duplicate', 'splash_hwnds'):
            with self.subTest(field=field):
                report = dict(self.report)
                report.pop(field)
                with self.assertRaisesRegex(RuntimeError, 'overlap a visible unaccepted trial'):
                    native.require_duplicate_trial(report)


if __name__ == '__main__':
    unittest.main()
