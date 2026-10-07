"""Inert reconstruction of the actual guest.sh/OpenSSH command composition."""
import shlex
import unittest

from tests.first_abort_cli.delivery_command import create_delivery_command


class DeliveryCommandTests(unittest.TestCase):
    # Public source observed at /home/kk/VMs/omarchy-dev/guest.sh. This is a
    # lexical model only: neither this wrapper nor any command below is run.
    GUEST_EXEC = (
        'exec ssh -o BatchMode=yes -o IdentitiesOnly=yes -o ConnectTimeout=5 '
        '-o UserKnownHostsFile=/home/kk/VMs/omarchy-dev/known_hosts '
        '-i /home/kk/VMs/omarchy-dev/ssh_ed25519 -p 22240 '
        'kdk_vm@127.0.0.1 "$@"'
    )

    def remote_argv(self, arguments):
        # The host's shell preserves guest.sh argv; "$@" preserves those
        # individual arguments again. SSH then joins remote argv with spaces.
        host = shlex.split(shlex.join(('/fixed/guest.sh', *arguments)))
        wrapper = shlex.split(self.GUEST_EXEC)
        self.assertEqual(wrapper[-1], '$@')
        ssh = [*wrapper[1:-1], *host[1:]]
        destination = ssh.index('kdk_vm@127.0.0.1')
        remote = ssh[destination + 1:]
        return remote, shlex.split(' '.join(remote))

    def test_semicolons_multiline_quotes_survive_one_remote_argument(self):
        sources = (
            "import os; from pathlib import Path; p=Path('/fixed/cache-v6'); p.mkdir(mode=0o700)",
            "value = 'single quote'; other = \"double quote\"\nassert value != other\n",
            "value = '''multiline\nquoted ' and \" text; $HOME `literal` $(literal)\\n'''\n",
            "# unicode and shell punctuation remain Python source\nvalue = 'тест; & | > < ( )'\n",
        )
        for source in sources:
            with self.subTest(source=source):
                argument = create_delivery_command(source)
                self.assertEqual(type(argument), tuple)
                self.assertEqual(len(argument), 1)
                remote, parsed = self.remote_argv(argument)
                self.assertEqual(remote, list(argument))
                self.assertEqual(parsed, ['/usr/bin/python3', '-I', '-B', '-c', source])

    def test_old_separate_ssh_argv_loses_the_python_source_boundary(self):
        source = "import os; from pathlib import Path; p=Path('/fixed/cache-v5'); p.mkdir(mode=0o700)"
        old = ['/usr/bin/python3', '-I', '-B', '-c', source]
        remote, parsed = self.remote_argv(old)
        self.assertEqual(remote, old)  # guest.sh "$@" itself is intact.
        self.assertNotEqual(parsed, old)  # SSH's remote shell loses the -c token.
        lexer = shlex.shlex(' '.join(remote), posix=True, punctuation_chars=';')
        self.assertIn(';', list(lexer))  # Python semicolons become shell syntax.

    def test_empty_nontext_nul_surrogate_and_oversized_source_refuse(self):
        for source in (None, True, b'pass', '', 'pass\0', '\ud800', 'x' * 32769, 'я' * 16385):
            with self.assertRaises(ValueError):
                create_delivery_command(source)
        self.assertEqual(len(create_delivery_command('x' * 32768)), 1)


if __name__ == '__main__':
    unittest.main()
