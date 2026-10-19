# Yahu for Android

Android shell for the existing Yahu web interface, with an independently bundled **GeckoView 158** engine. It does not render through the device's system WebView.

- Android 8.0+ / API 26; this build targets ARM64 devices.
- Application ID: `io.github.fffonion.yahu`.
- Persistent browser profile for login and local preferences.
- Android Back navigation, fullscreen, native single/multiple-choice prompts and document selection.
- Bottom system navigation is hidden while the app is focused; an edge gesture can reveal it temporarily. The normal status bar remains visible outside page fullscreen.
- Same-origin navigation stays in the app; other HTTP(S) links open in the system browser. Unsafe URL schemes and URL-embedded credentials are rejected. Certificate errors are not bypassed.
- The portable default is `http://localhost:9642`. Use ADB reverse when Yahu runs on the ADB host, or the connection-error address editor to enter your own server address. Saved addresses take precedence over the default.

## Docker build

Only Docker and Python 3 are needed on the host for building. Install ADB on the host to deploy; **do not install a host JDK or Android SDK**.

Run from the repository root. Set `ANDROID_OUT` and `ANDROID_SIGNING_DIR` privately if you want different output/signing locations; do not commit their resolved values.

```bash
ANDROID_OUT="${ANDROID_OUT:-$PWD/.android-build}"
ANDROID_SIGNING_DIR="${ANDROID_SIGNING_DIR:-$HOME/.local/share/yahu-android/signing}"
mkdir -p "$ANDROID_OUT/tmp" "$ANDROID_SIGNING_DIR"
chmod 700 "$ANDROID_SIGNING_DIR"
docker build -t yahu-android-gecko-builder:local -f android/Dockerfile android
docker run --rm --user "$(id -u):$(id -g)" \
  -e HOME=/out/home -e TMPDIR=/out/tmp \
  -v "$PWD:/src:ro" \
  -v "$ANDROID_OUT:/out" \
  -v "$ANDROID_SIGNING_DIR:/signing" \
  yahu-android-gecko-builder:local
APK="$ANDROID_OUT/yahu.apk"
```

`APK` points to the resulting signed package. The default output directory is ignored by Git.

The image contains JDK 21, Gradle 9.8.1, Android Gradle Plugin 9.4.1, Android SDK 37.2 and build-tools 36.0.0. GeckoView is pinned to `158.0.20261007201254` in `app/build.gradle`. Dependency metadata checks remain enabled.

The build runs native-wrapper checks and the plain-Java navigation tests, creates an unsigned APK with Gradle, aligns it and signs it. Gradle requires writable project directories, so the read-only source is copied into an output-only build snapshot; generated files and caches stay under `/out`.

The first build generates a private signing identity under the mounted signing directory. Retain that directory privately for APK upgrades. It is never included in the APK, Docker image or Git. The APK contains no service authentication key, cookie or password. This internal build is debuggable for ADB verification.

## Install and verify

Connect a device with ADB and select it through `ANDROID_SERIAL` if multiple devices are connected. For wireless ADB, supply your own address through `ADB_ENDPOINT` and run `adb connect "$ADB_ENDPOINT"`; keep device identifiers and addresses in your shell configuration. Set `APK` to the package you built or downloaded.

```bash
: "${APK:?Set APK to the package to install}"
adb install --no-incremental -r "$APK"
adb shell am start -W -n io.github.fffonion.yahu/.MainActivity
adb shell dumpsys package io.github.fffonion.yahu
```

When Yahu runs on the ADB host, enable the portable default connection before launching the app:

```bash
adb reverse tcp:9642 tcp:9642
```

For a server on another host, enter its address through the app's connection-error address editor instead.

Use `install -r` with the same private signing identity to preserve app data. GeckoView's profile is independent of Android WebView profiles and can require an initial sign-in.

`tools/remote_page.py` provides an ADB-forwarded Firefox Remote Debugging Protocol client for UI checks. It uses ADB's normal device selection, including `ANDROID_SERIAL`, unless a serial is explicitly supplied. It selects a visible document, rejects disconnected/oversized packets and removes its own forward on close. It contains no credentials and does not perform login. GeckoView uses the package's `firefox-debugger-socket`, not Chromium's CDP socket. Keep debugger replies and screenshots private when they contain session or account information.

## Web compatibility

The web frontend also targets Chromium 83 and avoids native `Array.at` / `String.replaceAll` dependencies. `WebCompat.Dockerfile` is an optional Ubuntu builder for the web binary. Mount the repository at `/src` and supply Rust/Bun executables and caches through your own container configuration. The Android app uses its bundled Gecko engine and does not require changing the device's system browser.

## Public-source privacy

Use environment parameters, repository-relative paths and documentation-only sample addresses in shared commands and fixtures. Keep actual device addresses, host filesystem locations, account names/IDs, credentials and build artifacts outside Git. Run `python3 scripts/check_privacy.py` from the repository root before committing.
