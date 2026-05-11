plugins {
    id("com.android.application") version "8.5.0"
    kotlin("android") version "2.0.0"
}

android {
    namespace = "glyphspace.host"
    compileSdk = 35
    defaultConfig {
        applicationId = "glyphspace.host"
        minSdk = 26
        targetSdk = 35
        versionCode = 1
        versionName = "0.1.0"
    }
}
