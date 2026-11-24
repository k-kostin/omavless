# SPDX-License-Identifier: MIT
"""Real committed-tree controls for the narrow developer-Markdown PR gate."""
import importlib.util
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import textwrap
import unittest
from unittest.mock import patch

ROOT = Path(__file__).resolve().parents[1]
SPEC = importlib.util.spec_from_file_location('ci_scope', ROOT / 'tests/ci_scope.py')
SCOPE = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(SCOPE)


class CheckScopeTests(unittest.TestCase):
    def setUp(self):
        parent = Path.home() / '.cache/ovt'
        parent.mkdir(parents=True, exist_ok=True)
        self.tmp = tempfile.TemporaryDirectory(prefix='ci-scope-', dir=parent)
        self.addCleanup(self.tmp.cleanup)
        self.repo = Path(self.tmp.name).resolve()
        self.git('init', '-q')
        self.git('config', 'user.name', 'Fixture')
        self.git('config', 'user.email', 'fixture@example.invalid')
        self.git('config', 'core.filemode', 'true')
        for name in SCOPE.DEVELOPER_MARKDOWN:
            self.write(name, 'original\n')
        self.write('crates/runtime.rs', 'original\n')
        self.write('tests/ci_scope.py', (ROOT / 'tests/ci_scope.py').read_text())
        self.base = self.commit()

    def git(self, *args):
        return subprocess.check_output(
            ['git', '-c', 'gc.auto=0', '-c', 'maintenance.auto=false',
             '-c', 'core.hooksPath=/dev/null', '-c', 'commit.gpgsign=false',
             '-c', 'init.templateDir=', '-C', str(self.repo), *args],
            timeout=15, stderr=subprocess.DEVNULL).decode().strip()

    def write(self, name, text):
        path = self.repo / name
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(text)

    def commit(self):
        self.git('add', '--all')
        self.git('commit', '-qm', 'fixture')
        return self.git('rev-parse', 'HEAD')

    def decision(self, base=None, head=None):
        return SCOPE.rust_required(self.repo, base or self.base, head or self.git('rev-parse', 'HEAD'))

    def test_all_allowed_regular_markdown_changes_omit_pr_rust(self):
        for name in SCOPE.DEVELOPER_MARKDOWN:
            self.write(name, 'changed\n')
        self.commit()
        self.assertEqual(self.decision(), (False, 'developer_markdown_only'))

    def test_product_mixed_unknown_and_user_help_changes_require_full(self):
        for name in ('crates/runtime.rs', 'docs/user/NATIVE_INSTALL.md',
                     'docs/development/unknown.md', '.github/workflows/test.yml'):
            with self.subTest(name=name):
                self.write(name, 'changed\n')
                self.write('AGENTS.md', 'changed\n')
                self.commit()
                self.assertTrue(self.decision()[0])

    def test_diverged_head_cannot_hide_product_change_in_newer_event_base(self):
        self.write('crates/runtime.rs', 'newer base\n')
        event_base = self.commit()
        self.git('checkout', '--detach', self.base)
        self.write('AGENTS.md', 'docs only relative to merge base\n')
        head = self.commit()
        self.assertFalse(self.decision(self.base, head)[0])
        self.assertTrue(self.decision(event_base, head)[0])

    def test_both_sides_of_rename_are_checked(self):
        (self.repo / 'AGENTS.md').unlink()
        self.git('mv', 'crates/runtime.rs', 'AGENTS.md')
        self.commit()
        self.assertTrue(self.decision()[0])

    def test_allowed_markdown_deletion_and_addition_keep_isolated_only(self):
        (self.repo / 'AGENTS.md').unlink()
        deleted = self.commit()
        self.assertFalse(self.decision()[0])
        self.write('AGENTS.md', 'new instructions\n')
        self.commit()
        self.assertFalse(self.decision(base=deleted)[0])

    def test_malformed_or_truncated_git_output_requires_full(self):
        original = SCOPE.git
        for bad in (b'not terminated', b'only-one-record\0', b'bad\0AGENTS.md\0',
                    b'\xff\0AGENTS.md\0', b'x' * 65537):
            def git(root, *args):
                return bad if args[0] == 'diff' else original(root, *args)
            with self.subTest(bytes=len(bad)), patch.object(SCOPE, 'git', side_effect=git):
                self.assertTrue(self.decision()[0])

    def test_executable_and_symlink_markdown_require_full(self):
        (self.repo / 'AGENTS.md').chmod(0o755)
        executable_base = self.commit()
        self.assertTrue(self.decision()[0])
        (self.repo / 'AGENTS.md').chmod(0o644)
        self.commit()
        self.assertTrue(self.decision(base=executable_base)[0])
        self.git('checkout', '--detach', self.base)
        (self.repo / 'AGENTS.md').unlink()
        (self.repo / 'AGENTS.md').symlink_to('CONTRIBUTING.md')
        self.commit()
        self.assertTrue(self.decision()[0])

    def test_empty_missing_base_invalid_id_and_wrong_checkout_require_full(self):
        self.assertTrue(self.decision()[0])
        for base in ('0' * 40, '--help', self.base.upper()):
            with self.subTest(base=base):
                self.assertTrue(self.decision(base=base)[0])
        self.write('AGENTS.md', 'changed\n')
        self.commit()
        self.assertTrue(self.decision(head=self.base)[0])

    def step_script(self, name):
        workflow = (ROOT / '.github/workflows/test.yml').read_text()
        section = workflow.split('      - name: ' + name + '\n', 1)[1].split('\n      - name:', 1)[0]
        return textwrap.dedent(section.split('        run: |\n', 1)[1])

    def scope_step(self, base, head, base_ref='development'):
        output = self.repo / 'scope-output'
        env = dict(os.environ, EVENT_BASE=base, EVENT_HEAD=head, EVENT_BASE_REF=base_ref,
                   RUNNER_TEMP=str(self.repo), GITHUB_OUTPUT=str(output))
        result = subprocess.run(['/bin/bash', '-c', self.step_script('Select PR check scope')], cwd=self.repo,
                                env=env, capture_output=True, text=True, timeout=15)
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertFalse(list(self.repo.glob('ov-ci-scope.*.py')))
        return output.read_text()

    def test_real_cli_from_base_does_not_execute_changed_candidate_classifier(self):
        self.write('tests/ci_scope.py', "raise SystemExit('candidate executed')\n")
        head = self.commit()
        self.assertEqual(self.scope_step(self.base, head),
                         'rust_required=true\nreason=outside_developer_markdown\n')

    def test_workflow_missing_base_classifier_falls_back_to_full(self):
        (self.repo / 'tests/ci_scope.py').unlink()
        base_without_classifier = self.commit()
        self.write('AGENTS.md', 'changed\n')
        head = self.commit()
        self.assertEqual(self.scope_step(base_without_classifier, head),
                         'rust_required=true\nreason=base_classifier_unavailable\n')

    def test_intermediate_base_classifier_is_not_read_or_executed(self):
        self.write('tests/ci_scope.py',
                   "from pathlib import Path\nPath('unaccepted-base-executed').touch()\n"
                   "print('rust_required=false')\nprint('reason=unsafe-base')\n")
        unaccepted_base = self.commit()
        self.write('AGENTS.md', 'changed\n')
        head = self.commit()
        self.assertEqual(self.scope_step(unaccepted_base, head, 'dev/stacked'),
                         'rust_required=true\nreason=unaccepted_base\n')
        self.assertFalse((self.repo / 'unaccepted-base-executed').exists())

    def test_actual_checkout_guard_rejects_missing_or_wrong_pr_head(self):
        for event, head, sha, expected in (
                ('pull_request', self.base, self.base, 0),
                ('push', '', self.base, 0),
                ('pull_request', '', self.base, 1),
                ('pull_request', '0' * 40, self.base, 1),
                ('pull_request', '--help', self.base, 1)):
            with self.subTest(event=event, head=head):
                env = dict(os.environ, GITHUB_EVENT_NAME=event, EVENT_HEAD=head, EVENT_SHA=sha)
                result = subprocess.run(['/bin/bash', '-c', self.step_script('Confirm exact source checkout')],
                                        cwd=self.repo, env=env, capture_output=True, timeout=15)
                self.assertEqual(result.returncode, expected)

    def test_real_cli_allows_exact_doc_only_head(self):
        self.write('AGENTS.md', 'changed\n')
        head = self.commit()
        result = subprocess.run([sys.executable, '-I', '-B', str(self.repo / 'tests/ci_scope.py'),
                                 self.base, head], cwd=self.repo, capture_output=True,
                                text=True, timeout=15)
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(result.stdout, 'rust_required=false\nreason=developer_markdown_only\n')
        self.assertEqual(self.scope_step(self.base, head), result.stdout)

    def test_workflow_keeps_pushes_full_and_admits_only_explicit_pr_false(self):
        workflow = (ROOT / '.github/workflows/test.yml').read_text()
        self.assertIn("if: github.event_name != 'pull_request' || steps.scope.outputs.rust_required != 'false'", workflow)
        self.assertIn('git show "$EVENT_BASE:tests/ci_scope.py" > "$scope_script"', workflow)
        self.assertIn('python3 -I -B "$scope_script" "$EVENT_BASE" "$EVENT_HEAD"', workflow)
        self.assertIn("'rust_required=true' 'reason=base_classifier_unavailable'", workflow)
        self.assertIn("if: github.event_name == 'push' && github.ref == 'refs/heads/development'", workflow)
        self.assertIn('run: test "$GITHUB_EVENT_NAME" = push && test "$GITHUB_REF" = refs/heads/development', workflow)
        self.assertIn('fetch-depth: 0', workflow)
        self.assertLess(workflow.index('name: Confirm exact source checkout'), workflow.index('name: Select PR check scope'))
        self.assertLess(workflow.index('name: Select PR check scope'), workflow.index('name: Run isolated checks'))
        self.assertNotIn('pull_request_target', workflow)
        self.assertNotIn('paths-ignore:', workflow)


class FixturePreparationTests(unittest.TestCase):
    def test_cold_home_without_cache_can_prepare_and_retire_actual_fixture(self):
        with tempfile.TemporaryDirectory(prefix='ci-cold-home-', dir=Path.home() / '.cache/ovt') as temp:
            home = Path(temp) / 'home'
            home.mkdir()
            self.assertFalse((home / '.cache').exists())
            with patch.object(Path, 'home', return_value=home):
                case = CheckScopeTests('test_all_allowed_regular_markdown_changes_omit_pr_rust')
                try:
                    case.setUp()
                    self.assertTrue((case.repo / '.git').is_dir())
                finally:
                    case.doCleanups()
            self.assertFalse(list((home / '.cache/ovt').glob('ci-scope-*')))


if __name__ == '__main__':
    unittest.main()
