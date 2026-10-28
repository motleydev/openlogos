// Minimal JVM test host for the UniFFI Kotlin bindings (B061). Not the Android app: it only
// proves that the generated Kotlin loads the Rust core through JNA and behaves like the Rust
// tests, without the Android SDK. scripts/kotlin-bindings.sh builds the library, generates
// the bindings into generated/kotlin (gitignored) and runs `gradle test`.
pluginManagement {
    repositories {
        gradlePluginPortal()
        mavenCentral()
    }
}

dependencyResolutionManagement {
    repositories {
        mavenCentral()
    }
}

rootProject.name = "CoreHost"
