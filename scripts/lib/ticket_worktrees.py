#!/usr/bin/env python3
"""Conservative ticket worktree evidence and preservation helpers.

This module does not decide whether an agent is abandoned or change ticket state.
"""
import argparse
import datetime
import fcntl
import json
import os
from pathlib import Path
import re
import subprocess
import sys


class Failure(Exception):
    def __init__(self, message, code=3):
        super().__init__(message)
        self.code = code


def run(args, cwd):
    try:
        result = subprocess.run(args, cwd=cwd, text=True, capture_output=True, check=False)
    except OSError as error:
        raise Failure(f"Cannot run {args[0]}: {error}") from error
    if result.returncode:
        raise Failure(f"{' '.join(args)} failed: {result.stderr.strip() or result.stdout.strip()}")
    return result.stdout


def git(root, *args):
    return run(['git', *args], root)


def team_key(root):
    try:
        text = (root / '.claude/team.md').read_text()
    except OSError as error:
        raise Failure(f"Cannot read team configuration: {error}") from error
    keys = set()
    for line in text.splitlines():
        match = re.match(r'^\s*(?:[-*]\s*)?(?:team[ _]key|team[ _]identifier)\s*:\s*[\"\']?([A-Za-z][A-Za-z0-9]*)[\"\']?\s*$', line.replace('**', ''), re.I)
        if match:
            keys.add(match.group(1).upper())
    if len(keys) != 1:
        raise Failure('Expected exactly one Team key/team_key in .claude/team.md')
    return keys.pop()


def ticket_key(value, team):
    match = re.fullmatch(rf'(?:{re.escape(team)}-)?([1-9][0-9]*)', value, re.I)
    if not match:
        raise Failure(f'Expected {team}-N or positive ticket number, got {value!r}', 2)
    return f'{team}-{match.group(1)}'


def matches(branch, ticket):
    team, number = ticket.rsplit('-', 1)
    return re.search(rf'(?<![a-z0-9]){re.escape(team)}-?{number}(?![0-9])', branch, re.I) is not None


def github_repo(root, remote):
    url = git(root, 'config', '--get', f'remote.{remote}.url').strip()
    match = re.fullmatch(r'(?:git@github\.com:|ssh://git@github\.com/|https://github\.com/)([A-Za-z0-9_.-]+/[A-Za-z0-9_.-]+?)(?:\.git)?/?', url)
    if not match:
        raise Failure(f'Remote {remote} is not an identifiable GitHub repository: {url}')
    return match.group(1)


def pull_requests(root, remote):
    repo = github_repo(root, remote)
    raw = run(['gh', 'api', '--paginate', '--slurp', f'repos/{repo}/pulls?state=all&per_page=100'], root)
    try:
        pages = json.loads(raw)
        if not isinstance(pages, list) or any(not isinstance(page, list) for page in pages):
            raise ValueError('expected paginated array of arrays')
        rows = []
        for page in pages:
            for row in page:
                if (not isinstance(row, dict) or type(row.get('number')) is not int
                        or row.get('state') not in ('open', 'closed')
                        or not isinstance(row.get('head'), dict)
                        or not isinstance(row['head'].get('ref'), str) or not row['head']['ref']):
                    raise ValueError('malformed PR record')
                rows.append(row)
        return rows
    except (ValueError, TypeError) as error:
        raise Failure(f'Incomplete PR listing: {error}') from error


def refs(root, remote):
    lines = git(root, 'ls-remote', '--heads', remote).splitlines()
    result = {}
    for line in lines:
        match = re.fullmatch(r'([a-fA-F0-9]{40}|[a-fA-F0-9]{64})\trefs/heads/(.+)', line)
        if not match:
            raise Failure(f'Malformed remote ref: {line!r}')
        result[match.group(2)] = match.group(1)
    return result


def default_branch(root, remote):
    output = git(root, 'ls-remote', '--symref', remote, 'HEAD')
    names = re.findall(r'^ref: refs/heads/(.+)\tHEAD$', output, re.M)
    if len(names) != 1:
        raise Failure('Cannot determine remote default branch')
    return names[0]


def worktrees(root):
    raw = git(root, 'worktree', 'list', '--porcelain', '-z')
    entries = []
    current = {}
    for field in raw.split('\0'):
        if not field:
            if current:
                if 'path' not in current:
                    raise Failure('Malformed worktree listing')
                entries.append(current)
                current = {}
        elif field.startswith('worktree '):
            current['path'] = field[9:]
        elif field.startswith('branch refs/heads/'):
            current['branch'] = field[18:]
        elif field.startswith(('HEAD ', 'locked ', 'prunable ')) or field in ('detached', 'bare', 'locked', 'prunable'):
            continue
        else:
            raise Failure(f'Malformed worktree field: {field!r}')
    if current:
        raise Failure('Incomplete worktree listing')
    return entries


def marker_ticket(path):
    marker = path / 'STRANDED.md'
    if marker.is_symlink():
        raise Failure(f'Refusing symlink marker: {marker}')
    if not marker.exists():
        return None
    try:
        content = marker.read_text()
    except (OSError, UnicodeError) as error:
        raise Failure(f'Cannot read marker {marker}: {error}') from error
    keys = re.findall(r'^- Ticket:\s*([A-Za-z][A-Za-z0-9]*-[1-9][0-9]*)\s*$', content, re.M)
    if len(keys) != 1:
        raise Failure(f'Marker has no unambiguous ticket field: {marker}')
    return keys[0].upper()


def local_branches(root):
    return git(root, 'for-each-ref', '--format=%(refname:strip=2)', 'refs/heads').splitlines()


def check(root, args, ticket):
    excluded = set(args.ignore_branch)
    if args.self:
        if not matches(args.self, ticket):
            raise Failure('--self branch must match ticket', 2)
        excluded.add(args.self)
    hits, preserved, errors = [], [], []
    # Every independent listing is attempted; unknown evidence always wins.
    try:
        for row in pull_requests(root, args.remote):
            branch = row['head']['ref']
            if matches(branch, ticket) and branch not in excluded:
                hits.append(f"PR #{row['number']} ({row['state']}): {branch}")
    except Failure as error:
        errors.append(str(error))
    try:
        for branch in refs(root, args.remote):
            if branch.startswith('stranded/') and matches(branch[9:], ticket):
                preserved.append(f'remote branch: {branch}')
            elif matches(branch, ticket) and branch not in excluded:
                hits.append(f'remote branch: {branch}')
    except Failure as error:
        errors.append(str(error))
    tombstoned = set()
    try:
        for tree in worktrees(root):
            branch = tree.get('branch', '')
            if not matches(branch, ticket) or branch in excluded:
                continue
            marked = marker_ticket(Path(tree['path'])) == ticket
            if branch.startswith('stranded/') or marked:
                preserved.append(f"worktree {tree['path']}: {branch}")
                tombstoned.add(branch)
            else:
                hits.append(f"worktree {tree['path']}: {branch}")
    except Failure as error:
        errors.append(str(error))
    try:
        for branch in local_branches(root):
            if not matches(branch, ticket) or branch in excluded or branch in tombstoned:
                continue
            if branch.startswith('stranded/'):
                preserved.append(f'local branch: {branch}')
            else:
                hits.append(f'local branch: {branch}')
    except Failure as error:
        errors.append(str(error))
    if preserved:
        print('PRESERVED STRANDED WORK (available for salvage):')
        print('\n'.join(f'  ~ {entry}' for entry in preserved))
    if hits:
        print(f'IN FLIGHT: {ticket}')
        print('\n'.join(f'  - {entry}' for entry in hits))
    if errors:
        print(f'INCOMPLETE: failing closed; do not claim {ticket}')
        print('\n'.join(f'  ! {entry}' for entry in errors))
        return 3
    if hits:
        return 1
    print(f'CLEAR: {ticket}')
    return 0


def checked_tree(root, path, ticket, remote):
    path = Path(path).resolve(strict=True)
    actual = Path(git(path, 'rev-parse', '--show-toplevel').strip()).resolve()
    if actual != path:
        raise Failure('--worktree must be its root directory', 2)
    own_common = Path(git(root, 'rev-parse', '--path-format=absolute', '--git-common-dir').strip()).resolve()
    target_common = Path(git(path, 'rev-parse', '--path-format=absolute', '--git-common-dir').strip()).resolve()
    target_git = Path(git(path, 'rev-parse', '--path-format=absolute', '--git-dir').strip()).resolve()
    if own_common != target_common:
        raise Failure('Refusing worktree from another repository', 1)
    if target_git == target_common:
        raise Failure('Refusing primary worktree', 1)
    registered = [tree for tree in worktrees(root) if Path(tree['path']).resolve() == path]
    if len(registered) != 1 or not registered[0].get('branch'):
        raise Failure('Refusing detached or unregistered worktree', 1)
    branch = registered[0]['branch']
    if branch == default_branch(root, remote) or not matches(branch, ticket):
        raise Failure('Refusing default branch or branch that does not match ticket', 1)
    return path, branch


def no_pr(root, remote, branch):
    if any(row['head']['ref'] == branch for row in pull_requests(root, remote)):
        raise Failure(f'Branch {branch} has a PR; preserve it for merge-sweeper', 1)


def mark_worktree(root, args, ticket):
    path, branch = checked_tree(root, args.worktree, ticket, args.remote)
    no_pr(root, args.remote, branch)
    prior = marker_ticket(path)
    if prior:
        if prior != ticket:
            raise Failure('Refusing conflicting stranded marker', 1)
        print(f'ALREADY TOMBSTONED: {path}')
        return 0
    released = datetime.datetime.now(datetime.timezone.utc).isoformat(timespec='seconds')
    content = (f'# STRANDED WORKTREE — {ticket}\n\n'
               'Preserved for salvage. This is not a ticket claim. Delete this marker\n'
               'before adopting the worktree. Existing files and commits remain intact.\n\n'
               f'- Ticket: {ticket}\n- Released: {released}\n- Path: {path}\n- Branch: {branch}\n')
    if args.note:
        content += f'\n## Note\n\n{args.note}\n'
    # Exclusive creation refuses symlinks and concurrent/conflicting writes.
    try:
        with (path / 'STRANDED.md').open('x') as output:
            output.write(content)
    except OSError as error:
        raise Failure(f'Cannot create marker: {error}') from error
    print(f'Wrote tombstone: {path / "STRANDED.md"}')
    return 0


def mark_branch(root, args, ticket):
    branch = args.branch
    if branch.startswith('stranded/'):
        branch = branch[9:]
    if not matches(branch, ticket) or branch == default_branch(root, args.remote):
        raise Failure('Refusing default branch or branch that does not match ticket', 1)
    git(root, 'check-ref-format', f'refs/heads/{branch}')
    no_pr(root, args.remote, branch)
    archive = f'stranded/{branch}'
    def checked_out_trees():
        current = [tree for tree in worktrees(root) if tree.get('branch') == branch]
        for tree in current:
            path, _ = checked_tree(root, tree['path'], ticket, args.remote)
            if marker_ticket(path) != ticket:
                raise Failure(f'Checked-out branch requires matching stranded marker: {path}', 1)
        return current

    checked_out_trees()
    remote_refs = refs(root, args.remote)
    original = remote_refs.get(branch)
    saved = remote_refs.get(archive)
    if original:
        # An already-existing archive could disappear while Git optimizes its
        # same-SHA update away. Refuse rather than treating a no-op as a lock.
        if saved:
            raise Failure('Original and stranded remote refs both exist; refusing ambiguous recovery', 1)
        git(root, 'fetch', '--no-tags', args.remote, f'refs/heads/{branch}')
        fetched = git(root, 'rev-parse', 'FETCH_HEAD').strip()
        if fetched != original:
            raise Failure('Remote branch changed during fetch; retry after reassessing ownership', 1)
        no_pr(root, args.remote, branch)
        checked_out_trees()
        # The server creates the archive and deletes the original in ONE
        # transaction. Both leases must hold; remotes without atomic support
        # fail closed. A failed operation cannot leave neither copy present.
        git(root, 'push', '--atomic', f'--force-with-lease=refs/heads/{archive}:',
            f'--force-with-lease=refs/heads/{branch}:{original}', args.remote,
            f'{original}:refs/heads/{archive}', f':refs/heads/{branch}')
        after = refs(root, args.remote)
        if after.get(archive) != original or branch in after:
            raise Failure('Cannot verify atomic preservation result; reassess before retrying')
    elif not saved:
        raise Failure('Neither original nor preserved branch exists on remote', 1)
    else:
        # Ensure preserved commits exist locally before considering a local rename.
        git(root, 'fetch', '--no-tags', args.remote, f'refs/heads/{archive}')
    local = local_branches(root)
    trees = checked_out_trees()
    if branch in local and not trees:
        if archive in local:
            raise Failure('Existing local archive prevents rename; original local branch preserved', 1)
        git(root, 'branch', '-m', branch, archive)
    print(f'PRESERVED: {args.remote}/{archive}; checked-out marked worktrees remain intact')
    if args.note:
        print(f'Note: {args.note}')
    return 0


def parser():
    result = argparse.ArgumentParser(description=__doc__)
    result.add_argument('action', choices=('check', 'mark-worktree', 'mark-branch'))
    result.add_argument('ticket')
    result.add_argument('legacy_remote', nargs='?')
    result.add_argument('--remote', default=None)
    result.add_argument('--self')
    result.add_argument('--ignore-branch', action='append', default=[])
    result.add_argument('--worktree')
    result.add_argument('--branch')
    result.add_argument('--note', default='')
    return result


def main():
    args = parser().parse_args()
    root = Path(__file__).resolve().parents[2]
    try:
        if args.remote and args.legacy_remote:
            raise Failure('Specify remote once', 2)
        args.remote = args.remote or args.legacy_remote or 'origin'
        if not re.fullmatch(r'[A-Za-z0-9][A-Za-z0-9_.-]*', args.remote):
            raise Failure('Invalid remote name', 2)
        ticket = ticket_key(args.ticket, team_key(root))
        if args.action == 'check':
            if args.branch or args.worktree or args.note:
                raise Failure('Writer arguments are not valid for check', 2)
            return check(root, args, ticket)
        if args.self or args.ignore_branch:
            raise Failure('Checker arguments are not valid for writers', 2)
        if args.action == 'mark-worktree' and (not args.worktree or args.branch):
            raise Failure('mark-worktree requires --worktree and accepts no --branch', 2)
        if args.action == 'mark-branch' and (not args.branch or args.worktree):
            raise Failure('mark-branch requires --branch and accepts no --worktree', 2)
        # Serialize local preservation writers across all linked worktrees.
        common = Path(git(root, 'rev-parse', '--path-format=absolute', '--git-common-dir').strip())
        locks = common / 'ticket-worktree-locks'
        locks.mkdir(exist_ok=True)
        with (locks / f'{ticket}.lock').open('a') as lock:
            try:
                fcntl.flock(lock, fcntl.LOCK_EX | fcntl.LOCK_NB)
            except BlockingIOError as error:
                raise Failure('Another preservation operation owns this ticket', 1) from error
            if args.action == 'mark-worktree':
                return mark_worktree(root, args, ticket)
            return mark_branch(root, args, ticket)
    except Failure as error:
        print(f'ERROR: {error}', file=sys.stderr)
        return error.code
    except (OSError, UnicodeError, ValueError) as error:
        print(f'INCOMPLETE: {error}', file=sys.stderr)
        return 3


if __name__ == '__main__':
    sys.exit(main())
