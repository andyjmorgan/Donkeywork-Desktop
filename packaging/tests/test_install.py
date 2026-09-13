"""Filesystem-only installer checks. Never runs systemd or modifies the host."""
import hashlib
import os
from pathlib import Path
import subprocess
import tempfile
import unittest

INSTALL = Path(__file__).resolve().parents[1] / 'install.sh'
BINARIES = ('dwconsole-daemon', 'dwconsole', 'input-daemon', 'input-cli',
            'web-launch', 'socket-broker', 'console-web')


class InstallerTest(unittest.TestCase):
    def setUp(self):
        prior_umask = os.umask(0o022)
        self.addCleanup(os.umask, prior_umask)
        self.temp = tempfile.TemporaryDirectory(prefix='dwdesktop-install-test-')
        self.addCleanup(self.temp.cleanup)
        self.base = Path(self.temp.name)
        self.source = self.base / 'payload'
        self.root = self.base / 'root'
        for folder in ('bin', 'libexec', 'share/web', 'share/systemd', 'packaging'):
            (self.source / folder).mkdir(parents=True)
        (self.source / 'VERSION').write_text('test-1\n')
        (self.source / 'ARCH').write_text('amd64\n')
        (self.source / 'share/web/index.html').write_text('test\n')
        (self.source / 'share/systemd/donkeywork-desktop.target').write_text('[Unit]\nDescription=fixture\n')
        for name in BINARIES:
            path = self.source / 'bin' / name
            path.write_text('#!/bin/sh\nexit 0\n')
            path.chmod(0o755)
        self.manifest()

    def manifest(self):
        files = sorted(p for p in self.source.rglob('*') if p.is_file() and p.name != 'SHA256SUMS')
        (self.source / 'SHA256SUMS').write_text(''.join(
            hashlib.sha256(p.read_bytes()).hexdigest() + '  ' + str(p.relative_to(self.source)) + '\n'
            for p in files))

    def run_install(self, *args, success=True):
        result = subprocess.run(['bash', str(INSTALL), '--source', str(self.source),
            '--root', str(self.root), '--profile', 'vkms', '--listen', '192.168.69.21',
            *args], capture_output=True, text=True)
        if success:
            self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        else:
            self.assertNotEqual(result.returncode, 0, result.stdout + result.stderr)
        return result

    def test_dry_run_does_not_create_staging_root(self):
        self.run_install('--dry-run')
        self.assertFalse(self.root.exists())

    def test_install_and_idempotent_reuse_preserve_release(self):
        self.run_install()
        current = self.root / 'opt/donkeywork-desktop/current'
        self.assertEqual(os.readlink(current), 'releases/test-1')
        inode = (current / 'bin/dwconsole-daemon').stat().st_ino
        self.run_install()
        self.assertEqual((current / 'bin/dwconsole-daemon').stat().st_ino, inode)
        self.assertIn('FFMPEG=/usr/bin/ffmpeg', (self.root / 'etc/donkeywork-desktop/console.env').read_text())

    def test_existing_configuration_requires_explicit_replacement(self):
        self.run_install()
        config = self.root / 'etc/donkeywork-desktop/console.env'
        before = config.read_text()
        self.run_install('--listen', '192.168.69.22', success=False)
        self.assertEqual(config.read_text(), before)
        self.run_install('--listen', '192.168.69.22', '--replace-config')
        self.assertIn('LISTEN_IP=192.168.69.22', config.read_text())

    def test_tampered_source_and_same_version_are_rejected(self):
        self.run_install()
        (self.source / 'share/web/index.html').write_text('changed')
        self.run_install(success=False)
        self.manifest()
        self.run_install(success=False)

    def test_bad_addresses_and_injected_paths_are_rejected(self):
        for address in ('127.0.0.1', '8.8.8.8', '192.168.1.999', '192.168.01.2', '192.168.0.1;true'):
            self.run_install('--listen', address, success=False)
        self.run_install('--ffmpeg', '/usr/bin/ffmpeg;true', success=False)
        self.run_install('--profile', 'kms', '--device', '/dev/dri/card0;true', success=False)
        self.run_install('--activate', success=False)
        self.assertFalse(self.root.exists())

    def test_symlink_payload_rejected(self):
        (self.source / 'unexpected').symlink_to('/etc')
        self.run_install(success=False)
        self.assertFalse(self.root.exists())

    def test_writable_or_symlink_destination_ancestors_rejected(self):
        self.root.mkdir()
        (self.root / 'etc').mkdir(mode=0o777)
        (self.root / 'etc').chmod(0o777)
        self.run_install(success=False)
        (self.root / 'etc').chmod(0o755)
        (self.root / 'opt').symlink_to(self.base / 'other')
        self.run_install(success=False)

    def test_restrictive_caller_umask_keeps_assets_traversable(self):
        os.umask(0o077)
        self.run_install()
        release = self.root / 'opt/donkeywork-desktop/current'
        for relative in ('.', 'share', 'share/web', 'bin'):
            self.assertEqual((release / relative).stat().st_mode & 0o777, 0o755)
        self.assertEqual((release / 'share/web/index.html').stat().st_mode & 0o444, 0o444)

    def test_vkms_conflict_fails_before_installing_any_release(self):
        vendor = self.root / 'usr/lib/udev/rules.d/61-mutter.rules'
        vendor.parent.mkdir(parents=True)
        vendor.write_text('ENV{ID_PATH}=="platform-vkms", TAG+="mutter-device-ignore"\n')
        local = self.root / 'etc/udev/rules.d/61-mutter.rules'
        local.parent.mkdir(parents=True)
        local.write_text('user rule\n')
        self.run_install('--configure-vkms', success=False)
        self.assertEqual(local.read_text(), 'user rule\n')
        self.assertFalse((self.root / 'opt').exists())

    def test_explicit_vkms_preserves_other_vendor_rules(self):
        vendor = self.root / 'usr/lib/udev/rules.d/61-mutter.rules'
        vendor.parent.mkdir(parents=True)
        vendor.write_text('other rule\nENV{ID_PATH}=="platform-vkms", TAG+="mutter-device-ignore"\nlast rule\n')
        self.run_install('--configure-vkms')
        configured = (self.root / 'etc/udev/rules.d/61-mutter.rules').read_text()
        self.assertIn('other rule\n', configured)
        self.assertIn('last rule\n', configured)
        self.assertNotIn('mutter-device-ignore', configured)
        self.assertIn('vkms\n', (self.root / 'etc/modules-load.d/donkeywork-desktop-vkms.conf').read_text())


if __name__ == '__main__':
    unittest.main()
