package io.github.fffonion.yahu;

public final class NavigationPolicyTest {
    private static int checks;

    private static void check(boolean condition, String label) {
        checks++;
        if (!condition) throw new AssertionError(label);
    }

    public static void main(String[] args) {
        check(NavigationPolicy.isWebUrl("http://10.0.0.52:9642"), "LAN HTTP endpoint");
        check(NavigationPolicy.isWebUrl("https://example.org/yahu"), "HTTPS endpoint");
        for (String url : new String[] {"javascript:alert(1)", "file:///sdcard/private", "data:text/html,test", "intent://example", "https://%5BREDACTED%5D:%5BREDACTED%5D@example.org", "http://", "http://example.org:99999", "not a URL", ""}) {
            check(!NavigationPolicy.isWebUrl(url), "Reject unsafe URL");
        }
        check(NavigationPolicy.sameOrigin("http://10.0.0.52:9642", "http://10.0.0.52:9642/#/usage"), "SPA navigation");
        check(NavigationPolicy.sameOrigin("https://EXAMPLE.org", "https://example.org:443/login"), "Canonical default port");
        check(!NavigationPolicy.sameOrigin("https://example.org", "http://example.org"), "Different scheme");
        check(!NavigationPolicy.sameOrigin("https://example.org", "https://example.org.evil.invalid"), "Host boundary");
        check(!NavigationPolicy.sameOrigin("http://10.0.0.52:9642", "http://10.0.0.52:8642"), "Port boundary");
        check(!NavigationPolicy.sameOrigin("https://example.org", "javascript:alert(1)"), "Unsafe destination");
        System.out.println("Navigation policy: " + checks + " checks passed");
    }
}
