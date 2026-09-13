import importlib.util
import os
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

ROOT = Path(__file__).resolve().parents[1]
spec = importlib.util.spec_from_file_location('x11_support', ROOT / 'x11_support.py')
support = importlib.util.module_from_spec(spec)
spec.loader.exec_module(support)

FIXTURE = '''Screen 0: minimum 8 x 8, current 1920 x 1080, maximum 32767 x 32767
HDMI-0 connected primary 1920x1080+0+0 (0x18d) normal (normal left inverted right x axis y axis) 708mm x 398mm
    Identifier: 0x18c
    Timestamp: 80555
    CRTC: 0
    Transform: 1.000000 0.000000 0.000000
               0.000000 1.000000 0.000000
               0.000000 0.000000 1.000000
              filter:
  1920x1080 (0x18d) 148.500MHz +HSync +VSync *current +preferred
USB-C-0 disconnected (normal left inverted right x axis y axis)
    Timestamp: 80555
'''


class X11GuardTests(unittest.TestCase):
    def test_supported_single_root_raster(self):
        self.assertEqual(support.parse_randr(FIXTURE, 1920, 1080), ('HDMI-0', '0x18d', '80555', '0'))

    def test_reject_geometry_scale_rotation_and_ambiguous_current(self):
        for text in (FIXTURE.replace('current 1920 x 1080', 'current 3840 x 1080'),
                     FIXTURE.replace('1920x1080+0+0', '1920x1080+1920+0'),
                     FIXTURE.replace('(0x18d) normal', '(0x18d) left'),
                     FIXTURE.replace('Transform: 1.000000', 'Transform: 2.000000'),
                     FIXTURE.replace(' *current', ''),
                     FIXTURE.replace('    CRTC: 0', '    CRTC: 0\n    Panning: 3840x1080+0+0')):
            with self.subTest(text=text):
                with self.assertRaises(ValueError):
                    support.parse_randr(text, 1920, 1080)

    def test_clone_monitor_is_not_a_single_output(self):
        extra = FIXTURE[FIXTURE.index('HDMI-0'):FIXTURE.index('USB-C-0')].replace('HDMI-0', 'HDMI-1')
        with self.assertRaises(ValueError):
            support.parse_randr(FIXTURE + extra, 1920, 1080)

    def test_mode_identity_changes_without_geometry_change(self):
        before = support.parse_randr(FIXTURE, 1920, 1080)
        after = support.parse_randr(FIXTURE.replace('80555', '80556'), 1920, 1080)
        self.assertNotEqual(before, after)

    def test_wrapped_signed_randr_timestamp(self):
        self.assertEqual(support.parse_randr(FIXTURE.replace('80555', '-481873'), 1920, 1080)[2], '-481873')

    def test_drm_dimensions_come_from_timings_not_name(self):
        with tempfile.TemporaryDirectory() as directory:
            state = Path(directory) / 'state'
            valid = 'crtc[80]: pipe A\n\tactive=1\n\tmode: "": 60 148500 1920 2008 2052 2200 1080 1084 1089 1125 0x0 0x5\n'
            for name in ('', '1920x1080', 'custom'):
                state.write_text(valid.replace('""', '"' + name + '"'))
                support.drm_snapshot(state, 1920, 1080)
            for invalid in (valid.replace(' 1920 ', ' 1280 '), valid.replace(' 1080 ', ' 720 '),
                            valid.replace(' 2008 ', ' 1900 '), valid + valid,
                            valid.replace('60 148500', 'bogus')):
                state.write_text(invalid)
                with self.assertRaises(ValueError):
                    support.drm_snapshot(state, 1920, 1080)

    def test_kms_connector_must_bind_captured_crtc(self):
        randr = FIXTURE.replace('    CRTC: 0', '    CRTC: 0\n    CONNECTOR_ID: 272')
        self.assertEqual(support.parse_randr(randr, 1920, 1080, True)[4], '272')
        with self.assertRaises(ValueError):
            support.parse_randr(FIXTURE, 1920, 1080, True)
        with tempfile.TemporaryDirectory() as directory:
            state = Path(directory) / 'state'
            valid = 'crtc[88]: pipe A\n\tactive=1\n\tmode: "": 60 148500 1920 2008 2052 2200 1080 1084 1089 1125 0x0 0x5\nconnector[272]: HDMI-A-2\n\tcrtc=pipe A\n'
            state.write_text(valid)
            support.drm_snapshot(state, 1920, 1080, '272')
            for invalid in (valid.replace('connector[272]', 'connector[273]'),
                            valid.replace('crtc=pipe A', 'crtc=pipe B'),
                            valid.replace('crtc=pipe A', 'crtc=(null)')):
                state.write_text(invalid)
                with self.assertRaises(ValueError):
                    support.drm_snapshot(state, 1920, 1080, '272')

    def test_no_remote_display_or_secondary_x_screen(self):
        for display in ('host:0', ':0.1', 'unix/:0', ':0;echo'):
            with self.assertRaises(ValueError):
                support.randr_snapshot(display, '/unused', 1920, 1080)

    def test_bounded_query_rejects_output_and_time_overrun(self):
        with self.assertRaises(ValueError):
            support.bounded_command(['/usr/bin/printf', '123456789'], {}, limit=4)
        with self.assertRaises(TimeoutError):
            support.bounded_command(['/usr/bin/sleep', '1'], {}, timeout=0.02)

    def test_xorg_discovery_uses_matching_vt_and_private_root_authority(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            process = root / '2262'
            process.mkdir()
            (process / 'comm').write_text('Xorg\n')
            (process / 'cmdline').write_bytes(b'/usr/lib/xorg/Xorg\0:0\0-auth\0/var/run/lightdm/root/:0\0vt7\0')
            session = {'session': 'c1', 'uid': 1000, 'display': ':0', 'vt': 7, 'locked': False}
            with patch.object(support, 'active_session', return_value=session), \
                 patch.object(support, 'authority_identity', return_value=('/run/lightdm/root/:0', 1, 2, 3, 4)), \
                 patch.object(support, 'bounded_command', return_value=FIXTURE):
                target = support.discover_target(width=1920, height=1080, proc=root)
                self.assertEqual(target['xauthority'], '/run/lightdm/root/:0')
                self.assertEqual(target['display'], ':0')
                session['vt'] = 8
                with self.assertRaises(ValueError):
                    support.discover_target(width=1920, height=1080, proc=root)

    def test_same_display_authority_alias_prefers_root_xorg(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            xorg = root / '22'
            xorg.mkdir()
            (xorg / 'comm').write_text('Xorg\n')
            (xorg / 'cmdline').write_bytes(b'Xorg\0:0\0-auth\0/server-auth\0vt7\0')
            desktop = root / '23'
            desktop.mkdir()
            (desktop / 'comm').write_text('gnome-shell\n')
            (desktop / 'environ').write_bytes(b'DISPLAY=:0\0XAUTHORITY=/user-auth\0')
            session = {'session': 'c1', 'uid': os.getuid(), 'display': ':0', 'vt': 7}
            real_stat = Path.stat
            def process_stat(path, *args, **kwargs):
                info = real_stat(path, *args, **kwargs)
                if path == xorg:
                    from types import SimpleNamespace
                    return SimpleNamespace(st_uid=0)
                return info
            with patch.object(support, 'active_session', return_value=session), \
                 patch.object(support, 'authority_identity', side_effect=lambda p, uid: (p, 1, 2, 3, 4)), \
                 patch.object(support, 'bounded_command', return_value=FIXTURE), \
                 patch.object(Path, 'stat', process_stat):
                self.assertEqual(support.discover_target(proc=root)['xauthority'], '/server-auth')


if __name__ == '__main__':
    unittest.main()
