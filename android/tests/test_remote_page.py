import importlib.util
import inspect
from pathlib import Path
import unittest
from unittest.mock import patch

spec = importlib.util.spec_from_file_location('remote_page', Path(__file__).resolve().parents[1] / 'tools/remote_page.py')
assert spec is not None and spec.loader is not None
remote_page = importlib.util.module_from_spec(spec)
spec.loader.exec_module(remote_page)


class RemotePageDeviceSelectionTests(unittest.TestCase):
    def test_default_uses_adb_device_selection(self):
        self.assertTrue(inspect.signature(remote_page.RemotePage).parameters['serial'].default is None, 'No device address may be hardcoded')
        page = remote_page.RemotePage.__new__(remote_page.RemotePage)
        page.serial = None
        with patch.object(remote_page.subprocess, 'check_output', return_value='ok\n') as command:
            self.assertEqual(page.adb('devices'), 'ok')
        command.assert_called_once_with(['adb', 'devices'], text=True)

    def test_explicit_serial_is_forwarded_without_a_default_address(self):
        page = remote_page.RemotePage.__new__(remote_page.RemotePage)
        page.serial = 'device-example'
        with patch.object(remote_page.subprocess, 'check_output', return_value='ok\n') as command:
            page.adb('get-state')
        command.assert_called_once_with(['adb', '-s', 'device-example', 'get-state'], text=True)


if __name__ == '__main__':
    unittest.main()
