# Android Wrapper Implementation Plan

**Goal:** Build the Yahu Android app in Docker, install it on an operator-selected ADB device, and validate the actual device UI.

**Architecture:** A Java Activity embeds the independently bundled GeckoView 158 engine. Same-origin navigation stays in the app; external HTTP(S) links use the system browser. The engine profile retains sign-in and browser preferences. Android Back, native choice prompts, document selection and page fullscreen are handled by explicit delegates. The bottom system navigation is hidden by app-local immersive flags. No credentials are embedded in the APK or source.

**Tech Stack:** Java 8-compatible source, JDK 21 in Docker, Android SDK 37.2 / build-tools 36.0.0, AGP 9.4.1, Gradle 9.8.1, API 26 minimum / API 30 target, ARM64, GeckoView `158.0.20261007201254`.

## Device and service configuration

- Select the test device through ADB's standard `ANDROID_SERIAL` configuration; do not embed a device address in scripts or documentation.
- Verify on a supported ARM64 Android device, including tablet layouts. The app uses bundled GeckoView independently of the system WebView version.
- The portable endpoint is `http://localhost:9642`, usable with ADB reverse when the service runs on the ADB host. Other server addresses are entered through the connection-error editor and retained privately in app preferences.
- App ID: `io.github.fffonion.yahu`; launcher name: `Yahu`.
- Host tools: Docker, existing Python 3 and ADB. No host JDK or Android SDK.

## Implementation and acceptance

1. Keep Android sources, manifest, native prompt presenter, URL policy, tests and Docker/Gradle scripts under `android/`.
2. Build, test, align and sign inside Docker. Supply host-specific output and signing locations through `ANDROID_OUT` and `ANDROID_SIGNING_DIR`; retain the private signing identity outside Git for compatible upgrades.
3. Read back installed version and launcher through ADB. Verify the real engine's user agent, authenticated DOM rendering, current CSS support, fullscreen transitions, Back navigation and login continuity after reopening. Use screenshots alongside DOM geometry. Send no test chat messages.
4. Present HTML choice prompts through Android dialogs, preserving selected/disabled/grouped options and cancellation. Confirm a real Settings selector opens on the device.
5. Keep authentication keys, cookies, passwords, signing material, APKs, build caches and identifying host/device/account data out of Git. Run the repository privacy check and inspect staged text for credential values without printing them.
6. See `android/README.md` for reproducible parameterized build/install commands.
