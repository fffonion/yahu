import unittest
from pathlib import Path

from check_privacy import privacy_issues, scan_repository


class PrivacyChecks(unittest.TestCase):
    def test_private_environment_literals_are_detected(self):
        cases = [
            ('http://' + '.'.join(('10', '24', '68', '9')) + ':9642', 'private-network-ip'),
            ('/'.join(('', 'home', 'operator', 'project')), 'host-specific-path'),
            ('/'.join(('', 'mnt', 'host-volume', 'build')), 'host-specific-path'),
            ('[Example User|' + str(987654321) + ']', 'platform-account-id'),
        ]
        for text, category in cases:
            with self.subTest(category=category):
                self.assertIn((1, category), privacy_issues(text))

    def test_tracked_sources_are_public_safe(self):
        findings = scan_repository(Path(__file__).resolve().parents[1])
        self.assertFalse(findings, f'{len(findings)} tracked privacy findings; run check_privacy.py for locations')

    def test_portable_parameters_and_documentation_addresses_are_allowed(self):
        for text in ('$HOME/.local/bin/yahu', '$ANDROID_OUT/yahu.apk',
                     'http://localhost:9642', 'http://127.0.0.1:9642',
                     'http://192.0.2.10:9642', '/workspace/example/src/main.rs',
                     '[Example User|10001]'):
            with self.subTest(text=text):
                self.assertEqual(privacy_issues(text), [])


if __name__ == '__main__':
    unittest.main()
