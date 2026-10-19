# Android Wrapper Implementation Plan

**Goal:** Build the Yahu Android app in Docker, install it through ADB on `10.0.0.56:5555`, and validate it on the actual tablet.

**Architecture:** A Java Activity embeds the independently bundled GeckoView 158 engine. Same-origin navigation stays in the app; external HTTP(S) links use the system browser. The engine profile retains sign-in and browser preferences. Android Back, native choice prompts, document selection and page fullscreen are handled by explicit delegates. The bottom system navigation is hidden by app-local immersive flags. No credentials are embedded in the APK or source.

**Tech Stack:** Java 8-compatible source, JDK 21 in Docker, Android SDK 37.2 / build-tools 36.0.0, AGP 9.4.1, Gradle 9.8.1, API 26 minimum / API 30 target, ARM64, GeckoView `158.0.20261007201254`.

## Device and service

- Target: `10.0.0.56:5555`, GM800, Android 11 / API 30, 1280×800.
- The system WebView is Chromium 83; the app uses bundled GeckoView instead of this renderer.
- Default LAN endpoint: `http://10.0.0.52:9642`; changeable through the connection-error address editor.
- App ID: `io.github.fffonion.yahu`; launcher name: `Yahu`.
- Host tools: Docker, existing Python 3 and ADB. No host JDK or Android SDK.

## Implementation and acceptance

1. Keep Android sources, manifest, native prompt presenter, URL policy, tests and Docker/Gradle scripts under `android/`.
2. Build, test, align and sign inside Docker. Intermediate outputs stay under `/mnt/TEMP/workspace/yahu-android`; retain the private signing identity outside Git for compatible upgrades.
3. Read back installed version and launcher through ADB. Verify the real engine's user agent, authenticated DOM rendering, current CSS support, fullscreen transitions, Back navigation and login continuity after reopening. Use screenshots alongside DOM geometry. Send no test chat messages.
4. Present HTML choice prompts through Android dialogs, preserving selected/disabled/grouped options and cancellation. Confirm the language selector opens on the device.
5. Keep authentication keys, cookies, passwords, signing material, APKs and build caches out of Git. Check the complete staged text against configured secret values without printing them.
6. Commit web-compatibility and Android-app scopes separately. See `android/README.md` for reproducible build/install commands.
