FROM eclipse-temurin:21-jdk-jammy
RUN apt-get update -qq \
    && apt-get install -y --no-install-recommends curl unzip python3 openssl \
    && rm -rf /var/lib/apt/lists/*
ENV ANDROID_HOME=/opt/android-sdk
ENV ANDROID_SDK_ROOT=/opt/android-sdk
RUN mkdir -p /opt/android-sdk/cmdline-tools \
    && curl -fL --retry 3 --max-time 240 https://dl.google.com/android/repository/commandlinetools-linux-16111833_latest.zip -o /opt/android-cli.zip \
    && unzip -q /opt/android-cli.zip -d /opt/android-sdk/cmdline-tools \
    && mv /opt/android-sdk/cmdline-tools/cmdline-tools /opt/android-sdk/cmdline-tools/latest \
    && rm /opt/android-cli.zip \
    && yes | /opt/android-sdk/cmdline-tools/latest/bin/sdkmanager --sdk_root=/opt/android-sdk --licenses >/dev/null
RUN /opt/android-sdk/cmdline-tools/latest/bin/sdkmanager --sdk_root=/opt/android-sdk 'platforms;android-37.2' 'build-tools;36.0.0'
RUN chmod -R a+rX /opt/android-sdk \
    && chmod a+rx /opt/android-sdk/build-tools/36.0.0/aapt /opt/android-sdk/build-tools/36.0.0/zipalign /opt/android-sdk/build-tools/36.0.0/apksigner /opt/android-sdk/build-tools/36.0.0/d8
RUN curl -fL --retry 3 --max-time 240 https://downloads.gradle.org/distributions/gradle-9.8.1-bin.zip -o /opt/gradle.zip \
    && unzip -q /opt/gradle.zip -d /opt && rm /opt/gradle.zip
ENV PATH=/opt/gradle-9.8.1/bin:$PATH
ENV ANDROID_BUILD_TOOLS=/opt/android-sdk/build-tools/36.0.0
ENV GRADLE_USER_HOME=/out/gradle-home
WORKDIR /src
ENTRYPOINT ["python3", "android/build.py"]
