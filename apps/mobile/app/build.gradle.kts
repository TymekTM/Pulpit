plugins {
    id("com.android.application")
    id("org.jetbrains.kotlin.android")
    id("org.jetbrains.kotlin.plugin.compose")
    id("org.jetbrains.kotlin.plugin.serialization")
}

// Single version source: the workspace Cargo.toml. versionName and the
// hello frame's client version (V2Client.VERSION via BuildConfig) both
// derive from it, so the surfaces cannot drift apart again.
val workspaceVersion: String =
    file("../../../Cargo.toml").readText()
        .substringAfter("[workspace.package]")
        .let { Regex("(?m)^version\\s*=\\s*\"([^\"]+)\"").find(it) }
        ?.groupValues?.get(1)
        ?: throw GradleException("missing [workspace.package] version in the workspace Cargo.toml")

// Monotonic with the version, so `adb install -r` of a newer release is
// never a downgrade: 1.5.2 -> 10502.
val workspaceVersionCode: Int =
    workspaceVersion.split(".").map { it.toInt() }
        .let { (major, minor, patch) -> major * 10_000 + minor * 100 + patch }

// Release signing key. The tablet only takes an update signed with the
// key already installed on it, so the release workflow restores that key
// from a secret and names it here (PULPIT_KEYSTORE). An explicit config,
// not the implicit ~/.android/debug.keystore: CI builds that relied on the
// implicit path ignored the restored file and signed every release with a
// freshly generated key, so no update ever installed over the previous
// one. A missing or wrong file fails the build. Without the variable
// (local builds) the release type signs with the local debug key.
val deckKeystore: String? = System.getenv("PULPIT_KEYSTORE")?.takeIf { it.isNotBlank() }

android {
    namespace = "app.pulpit.mobile"
    compileSdk = 34

    defaultConfig {
        applicationId = "app.pulpit.mobile"
        // the target tablet (SM-T561, LineageOS) runs Android 7.1 (API 25)
        minSdk = 24
        targetSdk = 34
        versionCode = workspaceVersionCode
        versionName = workspaceVersion
    }

    signingConfigs {
        if (deckKeystore != null) {
            create("deck") {
                storeFile = file(deckKeystore)
                storePassword = System.getenv("PULPIT_KEYSTORE_PASSWORD") ?: "android"
                keyAlias = System.getenv("PULPIT_KEY_ALIAS") ?: "androiddebugkey"
                keyPassword = System.getenv("PULPIT_KEY_PASSWORD") ?: "android"
            }
        }
    }

    buildTypes {
        release {
            // R8: kotlinx-serialization and OkHttp ship their consumer
            // keep rules. Signed with the deck key in CI (see
            // deckKeystore above), the local debug key otherwise, so a
            // local release build installs over a local debug build.
            isMinifyEnabled = true
            isShrinkResources = true
            signingConfig = signingConfigs.getByName(if (deckKeystore != null) "deck" else "debug")
            proguardFiles(getDefaultProguardFile("proguard-android-optimize.txt"))
        }
    }
    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_17
        targetCompatibility = JavaVersion.VERSION_17
    }
    kotlinOptions {
        jvmTarget = "17"
    }
    buildFeatures {
        compose = true
        // exposes VERSION_NAME to V2Client.VERSION (single version source)
        buildConfig = true
    }
    // Golden fixtures shared with the Rust contract tests
    // (crates/proto/tests/fixtures) so both stacks parse the same wire
    // examples.
    sourceSets.getByName("test") {
        resources.srcDir("../../../crates/proto/tests/fixtures")
    }
}

dependencies {
    val composeBom = platform("androidx.compose:compose-bom:2026.09.00")
    implementation(composeBom)

    implementation("androidx.core:core-ktx:1.13.1")
    implementation("androidx.activity:activity-compose:1.9.2")
    implementation("androidx.lifecycle:lifecycle-runtime-ktx:2.8.6")
    implementation("androidx.lifecycle:lifecycle-viewmodel-compose:2.8.6")
    implementation("androidx.compose.ui:ui")
    implementation("androidx.compose.ui:ui-graphics")
    implementation("androidx.compose.material3:material3")
    implementation("androidx.compose.material:material-icons-extended")

    implementation("com.squareup.okhttp3:okhttp:4.12.0")
    implementation("org.jetbrains.kotlinx:kotlinx-serialization-json:1.7.2")
    implementation("org.jetbrains.kotlinx:kotlinx-coroutines-android:1.8.1")

    testImplementation("junit:junit:4.13.2")
}
