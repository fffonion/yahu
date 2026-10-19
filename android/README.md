# Yahu for Android

Android shell for the existing Yahu web interface, with an independently bundled **GeckoView 158** engine. It does not render through the device's system WebView.

- Android 8.0+ / API 26; this build targets ARM64 tablets.
- Application ID: `io.github.fffonion.yahu`.
- Persistent browser profile for login and local preferences.
- Android Back navigation, fullscreen, native single/multiple-choice prompts and document selection.
- Bottom system navigation is hidden while the app is focused; an edge gesture can reveal it temporarily. The normal status bar remains visible outside page fullscreen.
- Same-origin navigation stays in the app; other HTTP(S) links open in the system browser. Unsafe URL schemes and URL-embedded credentials are rejected. Certificate errors are not bypassed.
- Default server URL is in `app/res/values/strings.xml`. Connection errors offer a native address editor.

## Docker build

Only Docker and Python 3 are needed on the host for building. Install ADB on the host to deploy; **do not install a host JDK or Android SDK**.

Run from the repository root:

```bash
docker build -t yahu-android-gecko-builder:local -f android/Dockerfile android
mkdir -p /mnt/TEMP/workspace/yahu-android/tmp
mkdir -p "$HOME/.local/share/yahu-android/signing"
chmod 700 "$HOME/.local/share/yahu-android/signing"
docker run --rm --user "$(id -u):$(id -g)" \
  -e HOME=/out/home -e TMPDIR=/out/tmp \
  -v "$PWD:/src:ro" \
  -v /mnt/TEMP/workspace/yahu-android:/out \
  -v "$HOME/.local/share/yahu-android/signing:/signing" \
  yahu-android-gecko-builder:local
```

Output: `/mnt/TEMP/workspace/yahu-android/yahu.apk`.

The image contains JDK 21, Gradle 9.8.1, Android Gradle Plugin 9.4.1, Android SDK 37.2 and build-tools 36.0.0. GeckoView is pinned to `158.0.20261007201254` in `app/build.gradle`. Dependency metadata checks remain enabled.

The build runs native-wrapper checks and the plain-Java navigation tests, creates an unsigned APK with Gradle, aligns it and signs it. Gradle requires writable project directories, so the read-only source is copied into an output-only build snapshot; generated files and caches stay under `/out`.

The first build generates a private signing identity under the mounted signing directory. Retain that directory privately for APK upgrades. It is never included in the APK, Docker image or Git. The APK contains no service authentication key, cookie or password. This internal build is debuggable for ADB verification.

## Install and verify

```bash
adb connect 10.0.0.56:5555
adb -s 10.0.0.56:5555 install --no-incremental -r /mnt/TEMP/workspace/yahu-android/yahu.apk
adb -s 10.0.0.56:5555 shell am start -W -n io.github.fffonion.yahu/.MainActivity
adb -s 10.0.0.56:5555 shell dumpsys package io.github.fffonion.yahu
```

Use `install -r` with the same private signing identity to preserve app data. GeckoView's profile is independent of the old Android WebView profile, so the engine transition can require signing in once.

`tools/remote_page.py` provides an ADB-forwarded Firefox Remote Debugging Protocol client for local UI checks. It selects a visible document, rejects disconnected/oversized packets and removes its own forward on close. It does not contain credentials or perform login. GeckoView uses the package's `firefox-debugger-socket`, not Chromium's CDP socket. Keep debugger replies and screenshots private when they contain session or account information.

## Older system WebView compatibility

The web frontend also targets Chromium 83 and avoids native `Array.at` / `String.replaceAll` dependencies. `WebCompat.Dockerfile` is the optional Ubuntu builder used to validate and rebuild the existing Yahu web binary with the configured local Rust/Bun caches. The Android build above uses its own bundled Gecko engine and does not require changing the device's system browser.
