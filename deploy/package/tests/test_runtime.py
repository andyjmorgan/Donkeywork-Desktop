import importlib.util
from pathlib import Path
import tempfile
from types import SimpleNamespace
import unittest
from unittest.mock import patch

ROOT = Path(__file__).resolve().parents[1]
spec = importlib.util.spec_from_file_location('package_runner', ROOT / 'package-runner.py')
runner = importlib.util.module_from_spec(spec)
spec.loader.exec_module(runner)


class RuntimeTests(unittest.TestCase):
    def config(self, profile='vkms', extra=''):
        return runner.parse_config('PROFILE=' + profile + '\nLISTEN_IP=192.168.69.21\n' + extra)

    def test_defaults_use_versioned_payload(self):
        config = self.config()
        self.assertEqual(config['ASSETS'], '/opt/donkeywork-desktop/current/share/web')
        self.assertEqual(config['BIN_DIR'], '/opt/donkeywork-desktop/current/bin')
        self.assertEqual(config['WIDTH'], '1920')
        self.assertEqual(config['FFMPEG'], '/usr/bin/ffmpeg')
        self.assertEqual(self.config(extra='FFMPEG=/opt/private-ffmpeg/bin/ffmpeg')['FFMPEG'], '/opt/private-ffmpeg/bin/ffmpeg')
        self.assertEqual(self.config(extra='FFMPEG=/opt/ffmpeg+x264/ffmpeg')['FFMPEG'], '/opt/ffmpeg+x264/ffmpeg')

    def test_reject_shell_and_unknown_duplicate_configuration(self):
        for text in ('PROFILE=vkms\nLISTEN_IP=$(id)', 'PROFILE=vkms\nPROFILE=x11\nLISTEN_IP=192.168.1.1',
                     'PROFILE=vkms\nLISTEN_IP=192.168.1.1\nEVIL=value',
                     'PROFILE="vkms"\nLISTEN_IP=192.168.1.1',
                     'PROFILE=vkms\nLISTEN_IP=0.0.0.0', 'PROFILE=vkms\nLISTEN_IP=8.8.8.8'):
            with self.subTest(text=text):
                with self.assertRaises(ValueError):
                    runner.parse_config(text)

    def test_profile_gates(self):
        with self.assertRaises(ValueError):
            self.config('kms')
        with self.assertRaises(ValueError):
            self.config(extra='WIDTH=3840\nHEIGHT=2160')
        with self.assertRaises(ValueError):
            self.config(extra='BIN_DIR=/tmp/bin')
        for value in ('ffmpeg', '/usr/bin/../evil'):
            with self.assertRaises(ValueError):
                self.config(extra='FFMPEG=' + value)
        kms = self.config('kms', 'DRM_DEVICE=/dev/dri/card0')
        self.assertEqual(kms['DRM_DEVICE'], '/dev/dri/card0')
        self.assertIsNone(runner.command('session-supervisor', kms))
        self.assertIsNone(runner.command('session-supervisor', self.config('x11')))
        for config in (kms, self.config('x11')):
            with self.assertRaises(ValueError):
                runner.command('input', config)

    def test_vkms_discovery_requires_exactly_one_software_card(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            sysfs = root / 'drm'
            sysfs.mkdir()
            driver = root / 'drivers/vkms'
            driver.mkdir(parents=True)
            config = self.config()
            with self.assertRaises(ValueError):
                runner.choose_drm(config, sysfs)
            for name in ('card7', 'card8'):
                device = sysfs / name / 'device'
                device.mkdir(parents=True)
                (device / 'driver').symlink_to(driver)
                if name == 'card7':
                    self.assertEqual(runner.choose_drm(config, sysfs), Path('/dev/dri/card7'))
            with self.assertRaises(ValueError):
                runner.choose_drm(config, sysfs)

    def test_vkms_platform_without_driver_link(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            device = root / 'card2/device'
            device.mkdir(parents=True)
            platform = root / 'platform'
            platform.mkdir()
            (device / 'subsystem').symlink_to(platform)
            (device / 'uevent').write_text('MODALIAS=platform:vkms\n')
            self.assertEqual(runner.choose_drm(self.config(), root), Path('/dev/dri/card2'))
            (device / 'uevent').write_text('MODALIAS=platform:other\n')
            with self.assertRaises(ValueError):
                runner.choose_drm(self.config(), root)

    def test_web_input_only_for_vkms(self):
        with patch.object(runner, 'trusted_file', side_effect=str), patch.object(runner, 'wait_socket'):
            for profile, extra in (('vkms', ''), ('x11', ''), ('kms', 'DRM_DEVICE=/dev/dri/card0')):
                args = runner.command('web', self.config(profile, extra))
                self.assertEqual('--input-fd' in args, profile == 'vkms')
                self.assertEqual('--input-socket' in args, profile == 'vkms')
                self.assertIn('--capture-broker-fd', args)
                self.assertIn('/opt/donkeywork-desktop/current/bin/console-web', args)

    def test_explicit_x11_control_wiring(self):
        with patch.object(runner, 'trusted_file', side_effect=str), patch.object(runner, 'wait_socket'), \
             patch.object(runner, 'drm_paths', return_value=('/dev/dri/card1', '/sys/kernel/debug/dri/1/state')):
            for profile in ('x11', 'kms'):
                config = self.config(profile, 'CONTROL=x11\nDRM_DEVICE=/dev/dri/card1')
                self.assertIn('--input-fd', runner.command('web', config))
                self.assertIn('--guard-state', runner.command('input', config))
                supervisor = runner.command('session-supervisor', config)
                self.assertIn('/opt/donkeywork-desktop/current/libexec/x11-session-supervisor.py', supervisor)
                self.assertEqual('--drm-state' in supervisor, profile == 'kms')
        with self.assertRaises(ValueError):
            self.config('vkms', 'CONTROL=x11')
        with self.assertRaises(ValueError):
            self.config('x11', 'CONTROL=ungarded')

    def test_capture_passes_configured_codec_and_x11_device(self):
        with patch.object(runner, 'trusted_file', side_effect=str), \
             patch.object(runner, 'drm_paths', return_value=('/dev/dri/card7', '/state')), \
             patch.object(Path, 'stat', return_value=SimpleNamespace(st_mode=0o100755)):
            config = self.config('x11', 'DRM_DEVICE=/dev/dri/card7\nFFMPEG=/opt/private/bin/ffmpeg')
            self.assertEqual(runner.command('capture', config)[-2:], ['/opt/private/bin/ffmpeg', '/dev/dri/card7'])
            config = self.config(extra='FFMPEG=/opt/private/bin/ffmpeg')
            command = runner.command('capture', config)
            self.assertEqual(command[command.index('--ffmpeg') + 1], '/opt/private/bin/ffmpeg')

    def test_unit_names_and_restart_membership(self):
        web_unit = (ROOT / 'systemd/donkeywork-desktop-web.service').read_text()
        self.assertIn('User=root', web_unit)
        self.assertNotIn('\nRestrictAddressFamilies=', web_unit)
        self.assertIn('NoNewPrivileges=yes', web_unit)
        self.assertIn('donkeywork-desktop-input.service', web_unit)
        for unit in (ROOT / 'systemd').glob('*.service'):
            text = unit.read_text()
            self.assertIn('PartOf=donkeywork-desktop.target', text)
            self.assertIn('/current/libexec/package-runner.py ', text)
            self.assertNotIn('dwconsole-', text)
        supervisor = (ROOT / 'session-supervisor.py').read_text()
        self.assertNotIn('/run/dwconsole', supervisor)
        self.assertIn('--property=PartOf=donkeywork-desktop.target', supervisor)
        target = (ROOT / 'systemd/donkeywork-desktop.target').read_text()
        self.assertNotIn('donkeywork-desktop-input.service', target)


if __name__ == '__main__':
    unittest.main()
