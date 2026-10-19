import re
from pathlib import Path
import unittest

SOURCE = Path(__file__).resolve().parents[1] / 'app/src/io/github/fffonion/yahu/MainActivity.java'


class ImmersiveNavigationTests(unittest.TestCase):
    def test_default_endpoint_is_portable_loopback(self):
        import xml.etree.ElementTree as ET
        resources = ET.parse(SOURCE.parents[5] / 'res/values/strings.xml')
        endpoint = next(item.text for item in resources.getroot().findall('string') if item.get('name') == 'server_url')
        self.assertTrue(endpoint == 'http://localhost:9642', 'Default endpoint must use portable loopback')

    def test_native_select_prompts_have_an_android_presenter(self):
        source = SOURCE.read_text()
        self.assertIn('onChoicePrompt(GeckoSession current, ChoicePrompt prompt)', source)
        self.assertIn('choicePrompts.show(prompt)', source)
        self.assertIn('choicePrompts.dismiss()', source)

    def test_browser_engine_is_bundled_instead_of_system_webview(self):
        source = SOURCE.read_text()
        self.assertIn('org.mozilla.geckoview.GeckoView', source)
        self.assertNotIn('android.webkit.WebView', source)
        self.assertIn('USER_AGENT_MODE_MOBILE', source)
        gradle = SOURCE.parents[5] / 'build.gradle'
        self.assertIn('org.mozilla.geckoview:geckoview:158.0.20261007201254', gradle.read_text())

    def test_navigation_hides_at_start_and_when_focus_returns(self):
        source = SOURCE.read_text()
        creation = source.split('public void onCreate(', 1)[1].split('private boolean handleNavigation', 1)[0]
        self.assertIn('applySystemBars();', creation)
        focus = source.split('public void onWindowFocusChanged(', 1)[1].split('@Override', 1)[0]
        self.assertIn('if (focused) applySystemBars();', focus)

    def test_navigation_hiding_is_local_and_preserves_normal_status_bar(self):
        source = SOURCE.read_text()
        method = re.search(r'private void applySystemBars\(\) \{(.*?)\n    \}', source, re.S)
        self.assertIsNotNone(method)
        assert method is not None
        self.assertIn('View.SYSTEM_UI_FLAG_HIDE_NAVIGATION', method.group(1))
        self.assertIn('View.SYSTEM_UI_FLAG_IMMERSIVE_STICKY', method.group(1))
        self.assertIn('if (fullScreen)', method.group(1))
        self.assertNotIn('Settings.Global', source)
        self.assertNotIn('Runtime.getRuntime', source)


if __name__ == '__main__':
    unittest.main()
