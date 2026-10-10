#!/usr/bin/env python3
"""Behavioral preservation checks against real disposable Git repositories."""
import json
import os
from pathlib import Path
import shutil
import subprocess
import tempfile
import unittest

SOURCE = Path(__file__).resolve().parents[1]


class TicketWorktreeTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.base = Path(self.temp.name)
        self.repo = self.base / 'repo'
        self.repo.mkdir()
        self.remote = self.base / 'remote.git'
        self.env = os.environ.copy()
        self.env.update(GIT_AUTHOR_NAME='Test', GIT_AUTHOR_EMAIL='test@example.invalid',
                        GIT_COMMITTER_NAME='Test', GIT_COMMITTER_EMAIL='test@example.invalid',
                        GIT_CONFIG_NOSYSTEM='1', GIT_CONFIG_GLOBAL='/dev/null')
        self.git('init', '-b', 'main')
        (self.repo / '.claude').mkdir()
        (self.repo / '.claude/team.md').write_text('- **Team key:** FLA\n- **GitHub repo:** stale/wrong\n')
        (self.repo / 'README').write_text('initial\n')
        (self.repo / 'scripts/lib').mkdir(parents=True)
        for name, action in (('check-ticket-in-flight.sh', 'check'), ('mark-worktree-stranded.sh', 'mark-worktree'), ('mark-branch-stranded.sh', 'mark-branch')):
            # Build portable wrappers, independent of a destination's legacy checker.
            (self.repo / 'scripts' / name).write_text(f'#!/usr/bin/env bash\nexec python3 "$(dirname "$0")/lib/ticket_worktrees.py" {action} "$@"\n')
        shutil.copy2(SOURCE / 'lib/ticket_worktrees.py', self.repo / 'scripts/lib/ticket_worktrees.py')
        self.git('add', '.')
        self.git('commit', '-m', 'initial')
        self.cmd(['git', 'init', '--bare', '-b', 'main', str(self.remote)])
        self.git('remote', 'add', 'origin', 'https://github.com/example/project.git')
        self.git('config', f'url.{self.remote}.insteadOf', 'https://github.com/example/project.git')
        self.git('push', '-u', 'origin', 'main')
        self.prfile = self.base / 'prs.json'
        self.prfile.write_text('[[]]')
        self.log = self.base / 'gh.log'
        self.env.update(PR_FILE=str(self.prfile), GH_LOG=str(self.log))
        self.bin = self.base / 'bin'
        self.bin.mkdir()
        gh = self.bin / 'gh'
        gh.write_text('''#!/usr/bin/env python3
import os, sys
from pathlib import Path
with Path(os.environ['GH_LOG']).open('a') as f: f.write(' '.join(sys.argv[1:])+'\\n')
if os.environ.get('GH_FAIL'): print('API failed',file=sys.stderr); sys.exit(1)
if os.environ.get('RACE_MODE') and not Path(os.environ['RACE_SENTINEL']).exists():
    # The writer checks PRs once before and once after preservation verification.
    if len(Path(os.environ['GH_LOG']).read_text().splitlines()) == 2:
        import subprocess
        if os.environ['RACE_MODE'] == 'checkout':
            subprocess.run(['git','-C',os.environ['RACE_REPO'],'worktree','add',os.environ['RACE_TREE'],os.environ['RACE_BRANCH']],check=True,stdout=subprocess.DEVNULL)
            Path(os.environ['RACE_SENTINEL']).touch()
            print(Path(os.environ['PR_FILE']).read_text()); sys.exit(0)
        repo=os.environ['RACE_REPO']; branch=os.environ['RACE_BRANCH']
        subprocess.run(['git','-C',repo,'commit','--allow-empty','-m','concurrent push'],check=True,stdout=subprocess.DEVNULL)
        target = ('stranded/' + branch) if os.environ['RACE_MODE'] == 'archive' else branch
        subprocess.run(['git','-C',repo,'push','origin','HEAD:refs/heads/'+target],check=True,stdout=subprocess.DEVNULL)
        Path(os.environ['RACE_SENTINEL']).touch()
print(Path(os.environ['PR_FILE']).read_text())
''')
        gh.chmod(0o755)
        self.env['PATH'] = f'{self.bin}:{self.env["PATH"]}'

    def cmd(self, args, cwd=None, check=True):
        return subprocess.run(args, cwd=cwd or self.repo, env=self.env, text=True, capture_output=True, check=check)

    def git(self, *args, cwd=None):
        return self.cmd(['git', *args], cwd=cwd).stdout.strip()

    def helper(self, name, *args, code=0):
        result = self.cmd(['bash', str(self.repo / 'scripts' / f'{name}.sh'), *map(str, args)], check=False)
        self.assertEqual(result.returncode, code, result.stdout + result.stderr)
        return result

    def tree(self, branch='feature/fla-17-ticket'):
        path = self.base / branch.replace('/', '_')
        self.git('worktree', 'add', '-b', branch, str(path), 'main')
        return path

    def publish(self, path):
        self.git('push', 'origin', 'HEAD', cwd=path)

    def prs(self, *branches):
        self.prfile.write_text(json.dumps([[{'number': i + 1, 'state': 'open', 'head': {'ref': branch}} for i, branch in enumerate(branches)]]))

    def test_clear_and_origin_endpoint(self):
        self.helper('check-ticket-in-flight', 'FLA-17')
        self.assertIn('repos/example/project/pulls?state=all', self.log.read_text())
        self.assertIn('--paginate --slurp', self.log.read_text())

    def test_numeric_and_team_boundaries(self):
        for branch in ('feature/fla-170-other', 'feature/notfla-17-other', 'feature/fla170-other'):
            self.git('branch', branch)
        self.helper('check-ticket-in-flight', 'FLA-17')
        self.git('branch', 'feature/fla17compact')
        self.helper('check-ticket-in-flight', 'FLA-17', code=1)
        self.helper('check-ticket-in-flight', 'OTHER-17', code=2)
        self.helper('check-ticket-in-flight', '0', code=2)

    def test_explicit_self_only(self):
        path = self.tree()
        self.git('checkout', 'feature/fla-17-ticket', cwd=path)
        self.helper('check-ticket-in-flight', '17', code=1)
        self.helper('check-ticket-in-flight', '17', '--self', 'feature/fla-17-ticket')
        self.helper('check-ticket-in-flight', '17', '--self', 'feature/fla-170-other', code=2)

    def test_api_failure_and_malformed_response_fail_closed(self):
        self.env['GH_FAIL'] = '1'
        self.helper('check-ticket-in-flight', '17', code=3)
        del self.env['GH_FAIL']
        for raw in ('not-json', '[{}]', '[[{"number":1,"state":"open","head":{}}]]'):
            self.prfile.write_text(raw)
            self.helper('check-ticket-in-flight', '17', code=3)
        self.prfile.write_text('[[]]')
        self.git('remote', 'set-url', 'origin', '/does/not/exist')
        self.helper('check-ticket-in-flight', '17', code=3)

    def test_all_pr_states_and_pages_block(self):
        for state in ('open', 'closed'):
            self.prfile.write_text(json.dumps([[], [{'number': 999, 'state': state, 'head': {'ref': 'feature/fla-17-ticket'}}]]))
            self.helper('check-ticket-in-flight', '17', code=1)

    def test_marker_preserves_files_and_index_and_is_idempotent(self):
        path = self.tree()
        (path / 'README').write_text('dirty staged\n')
        self.git('add', 'README', cwd=path)
        (path / 'untracked.txt').write_text('salvage\n')
        before = self.git('diff', '--cached', cwd=path)
        self.helper('mark-worktree-stranded', '17', '--worktree', path, '--note', 'abandonment checked externally')
        self.assertEqual((path / 'README').read_text(), 'dirty staged\n')
        self.assertEqual((path / 'untracked.txt').read_text(), 'salvage\n')
        self.assertEqual(self.git('diff', '--cached', cwd=path), before)
        marker = (path / 'STRANDED.md').read_bytes()
        self.helper('mark-worktree-stranded', '17', '--worktree', path)
        self.assertEqual((path / 'STRANDED.md').read_bytes(), marker)
        self.helper('check-ticket-in-flight', '17')

    def test_marker_does_not_hide_remote_or_pr(self):
        path = self.tree()
        self.publish(path)
        self.helper('mark-worktree-stranded', '17', '--worktree', path)
        self.helper('check-ticket-in-flight', '17', code=1)
        self.prs('feature/fla-17-ticket')
        self.helper('check-ticket-in-flight', '17', code=1)
        self.helper('mark-branch-stranded', '17', '--branch', 'feature/fla-17-ticket', code=1)

    def test_refuse_primary_detached_wrong_ticket_and_foreign(self):
        self.helper('mark-worktree-stranded', '17', '--worktree', self.repo, code=1)
        path = self.tree('feature/fla-170-other')
        self.helper('mark-worktree-stranded', '17', '--worktree', path, code=1)
        self.git('checkout', '--detach', cwd=path)
        self.helper('mark-worktree-stranded', '170', '--worktree', path, code=1)
        foreign = self.base / 'foreign'
        foreign.mkdir()
        self.cmd(['git', 'init', '-b', 'main', str(foreign)])
        (foreign / 'README').write_text('foreign')
        self.git('add', '.', cwd=foreign)
        self.git('commit', '-m', 'foreign', cwd=foreign)
        fwt = self.base / 'foreign-tree'
        self.git('worktree', 'add', '-b', 'feature/fla-17-ticket', str(fwt), cwd=foreign)
        self.helper('mark-worktree-stranded', '17', '--worktree', fwt, code=1)

    def test_conflicting_and_symlink_markers(self):
        path = self.tree()
        marker = path / 'STRANDED.md'
        marker.write_text('- Ticket: FLA-170\n')
        self.helper('mark-worktree-stranded', '17', '--worktree', path, code=1)
        self.assertEqual(marker.read_text(), '- Ticket: FLA-170\n')
        marker.unlink()
        victim = self.base / 'victim'
        victim.write_text('do not touch')
        marker.symlink_to(victim)
        self.helper('mark-worktree-stranded', '17', '--worktree', path, code=3)
        self.assertEqual(victim.read_text(), 'do not touch')
        self.helper('check-ticket-in-flight', '17', code=3)

    def test_branch_archive_preserves_checked_out_dirty_work(self):
        path = self.tree()
        self.publish(path)
        (path / 'README').write_text('dirty')
        original = self.git('rev-parse', 'HEAD', cwd=path)
        self.helper('mark-branch-stranded', '17', '--branch', 'feature/fla-17-ticket', code=1)
        self.helper('mark-worktree-stranded', '17', '--worktree', path)
        self.helper('mark-branch-stranded', '17', '--branch', 'feature/fla-17-ticket')
        remote = self.git('ls-remote', '--heads', 'origin')
        self.assertIn(f'{original}\trefs/heads/stranded/feature/fla-17-ticket', remote)
        self.assertNotIn('\trefs/heads/feature/fla-17-ticket', remote)
        self.assertEqual((path / 'README').read_text(), 'dirty')
        self.assertEqual(self.git('branch', '--show-current', cwd=path), 'feature/fla-17-ticket')
        self.helper('mark-branch-stranded', '17', '--branch', 'feature/fla-17-ticket')
        self.helper('check-ticket-in-flight', '17')

    def test_branch_archive_local_rename_and_idempotence(self):
        self.git('branch', 'feature/fla-17-ticket')
        self.git('push', 'origin', 'feature/fla-17-ticket')
        self.helper('mark-branch-stranded', '17', '--branch', 'feature/fla-17-ticket')
        self.assertIn('stranded/feature/fla-17-ticket', self.git('branch', '--list'))
        self.helper('mark-branch-stranded', '17', '--branch', 'stranded/feature/fla-17-ticket')
        self.helper('check-ticket-in-flight', '17')

    def test_default_wrong_ticket_and_conflicting_archive_are_preserved(self):
        self.helper('mark-branch-stranded', '17', '--branch', 'main', code=1)
        self.helper('mark-branch-stranded', '17', '--branch', 'feature/fla-170-other', code=1)
        self.git('branch', 'feature/fla-17-ticket')
        self.git('push', 'origin', 'feature/fla-17-ticket')
        self.git('commit', '--allow-empty', '-m', 'different')
        self.git('push', 'origin', 'HEAD:refs/heads/stranded/feature/fla-17-ticket')
        self.helper('mark-branch-stranded', '17', '--branch', 'feature/fla-17-ticket', code=1)
        self.assertIn('refs/heads/feature/fla-17-ticket', self.git('ls-remote', '--heads', 'origin'))

    def test_concurrent_push_is_not_deleted(self):
        path = self.tree()
        self.publish(path)
        self.helper('mark-worktree-stranded', '17', '--worktree', path)
        self.log.unlink()
        original = self.git('rev-parse', 'HEAD', cwd=path)
        self.env.update(RACE_MODE='1', RACE_REPO=str(path), RACE_BRANCH='feature/fla-17-ticket', RACE_SENTINEL=str(self.base / 'race-done'))
        self.helper('mark-branch-stranded', '17', '--branch', 'feature/fla-17-ticket', code=3)
        newest = self.git('rev-parse', 'HEAD', cwd=path)
        self.assertNotEqual(newest, original)
        remote = self.git('ls-remote', '--heads', 'origin')
        self.assertIn(f'{newest}\trefs/heads/feature/fla-17-ticket', remote)
        self.assertNotIn('refs/heads/stranded/feature/fla-17-ticket', remote)

    def test_archive_race_does_not_delete_original(self):
        path = self.tree()
        self.publish(path)
        self.helper('mark-worktree-stranded', '17', '--worktree', path)
        self.log.unlink()
        original = self.git('rev-parse', 'HEAD', cwd=path)
        self.env.update(RACE_MODE='archive', RACE_REPO=str(path), RACE_BRANCH='feature/fla-17-ticket', RACE_SENTINEL=str(self.base / 'race-done'))
        self.helper('mark-branch-stranded', '17', '--branch', 'feature/fla-17-ticket', code=3)
        newest = self.git('rev-parse', 'HEAD', cwd=path)
        remote = self.git('ls-remote', '--heads', 'origin')
        self.assertIn(f'{original}\trefs/heads/feature/fla-17-ticket', remote)
        self.assertIn(f'{newest}\trefs/heads/stranded/feature/fla-17-ticket', remote)

    def test_same_sha_existing_archive_refuses_ambiguous_recovery(self):
        self.git('branch', 'feature/fla-17-ticket')
        self.git('push', 'origin', 'feature/fla-17-ticket')
        self.git('push', 'origin', 'feature/fla-17-ticket:refs/heads/stranded/feature/fla-17-ticket')
        self.helper('mark-branch-stranded', '17', '--branch', 'feature/fla-17-ticket', code=1)
        self.assertIn('refs/heads/feature/fla-17-ticket', self.git('ls-remote', '--heads', 'origin'))

    def test_remote_without_atomic_support_preserves_original(self):
        self.git('branch', 'feature/fla-17-ticket')
        self.git('push', 'origin', 'feature/fla-17-ticket')
        self.cmd(['git', '-C', str(self.remote), 'config', 'receive.advertiseAtomic', 'false'])
        self.helper('mark-branch-stranded', '17', '--branch', 'feature/fla-17-ticket', code=3)
        remote = self.git('ls-remote', '--heads', 'origin')
        self.assertIn('refs/heads/feature/fla-17-ticket', remote)
        self.assertNotIn('refs/heads/stranded/feature/fla-17-ticket', remote)

    def test_new_checkout_during_network_check_is_preserved(self):
        self.git('branch', 'feature/fla-17-ticket')
        self.git('push', 'origin', 'feature/fla-17-ticket')
        path = self.base / 'new-owner-tree'
        self.env.update(RACE_MODE='checkout', RACE_REPO=str(self.repo), RACE_TREE=str(path), RACE_BRANCH='feature/fla-17-ticket', RACE_SENTINEL=str(self.base / 'race-done'))
        self.helper('mark-branch-stranded', '17', '--branch', 'feature/fla-17-ticket', code=1)
        self.assertEqual(self.git('branch', '--show-current', cwd=path), 'feature/fla-17-ticket')
        self.assertIn('refs/heads/feature/fla-17-ticket', self.git('ls-remote', '--heads', 'origin'))

    def test_ticket_shaped_remote_default_branch_is_refused(self):
        self.git('branch', 'feature/fla17-default')
        self.git('push', 'origin', 'feature/fla17-default')
        self.cmd(['git', '-C', str(self.remote), 'symbolic-ref', 'HEAD', 'refs/heads/feature/fla17-default'])
        self.helper('mark-branch-stranded', '17', '--branch', 'feature/fla17-default', code=1)
        self.assertIn('refs/heads/feature/fla17-default', self.git('ls-remote', '--heads', 'origin'))

    def test_writer_api_failure_never_writes_or_deletes(self):
        path = self.tree()
        self.publish(path)
        self.env['GH_FAIL'] = '1'
        self.helper('mark-worktree-stranded', '17', '--worktree', path, code=3)
        self.assertFalse((path / 'STRANDED.md').exists())
        self.helper('mark-branch-stranded', '17', '--branch', 'feature/fla-17-ticket', code=3)
        self.assertIn('refs/heads/feature/fla-17-ticket', self.git('ls-remote', '--heads', 'origin'))

    def test_snake_case_team_and_streamr_compact_branch(self):
        (self.repo / '.claude/team.md').write_text('team_key: STR\n')
        self.git('branch', 'feature/str17-compact')
        self.helper('check-ticket-in-flight', 'STR-17', code=1)
        self.helper('check-ticket-in-flight', 'STR-170')


if __name__ == '__main__':
    unittest.main()
