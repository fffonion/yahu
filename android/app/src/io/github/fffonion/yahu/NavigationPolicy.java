package io.github.fffonion.yahu;

import java.net.URI;
import java.net.URISyntaxException;

final class NavigationPolicy {
    private static URI parse(String url) {
        if (url == null) return null;
        try {
            URI uri = new URI(url);
            String scheme = uri.getScheme();
            if (!("https".equalsIgnoreCase(scheme) || "http".equalsIgnoreCase(scheme))) return null;
            if (uri.getHost() == null || uri.getRawUserInfo() != null) return null;
            if (uri.getPort() < -1 || uri.getPort() == 0 || uri.getPort() > 65535) return null;
            return uri;
        } catch (URISyntaxException error) {
            return null;
        }
    }

    static boolean isWebUrl(String url) {
        return parse(url) != null;
    }

    private static int port(URI uri) {
        return uri.getPort() >= 0 ? uri.getPort() : ("https".equalsIgnoreCase(uri.getScheme()) ? 443 : 80);
    }

    static boolean sameOrigin(String serverUrl, String destination) {
        URI base = parse(serverUrl);
        URI target = parse(destination);
        return base != null && target != null
                && base.getScheme().equalsIgnoreCase(target.getScheme())
                && base.getHost().equalsIgnoreCase(target.getHost())
                && port(base) == port(target);
    }
}
