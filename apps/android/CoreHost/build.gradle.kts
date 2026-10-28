plugins {
    kotlin("jvm") version "2.2.0"
}

group = "org.openlogos"
version = "0.1.0"

// Versions are pinned here and in gradle.lockfile (`gradle dependencies --write-locks`).
dependencies {
    // The generated bindings call the Rust library through JNA.
    implementation("net.java.dev.jna:jna:5.17.0")
    testImplementation(kotlin("test"))
    testImplementation("org.junit.jupiter:junit-jupiter:5.12.2")
    testRuntimeOnly("org.junit.platform:junit-platform-launcher")
    // Reads the conformance fixtures JSON in the tests.
    testImplementation("org.json:json:20250517")
}

dependencyLocking {
    lockAllConfigurations()
}

kotlin {
    // JDK 21 from `nix develop .#android`; no toolchain download.
    jvmToolchain(21)
}

sourceSets {
    main {
        // scripts/kotlin-bindings.sh writes org/openlogos/core/openlogos_ffi.kt here.
        kotlin.srcDir("generated/kotlin")
    }
}

tasks.test {
    useJUnitPlatform()
    // Where JNA finds libopenlogos_ffi.dylib; scripts/kotlin-bindings.sh passes the release
    // directory of the host target.
    val libDir = (findProperty("openlogosLibDir") as String?)
        ?: rootDir.resolve("../../../target/aarch64-apple-darwin/release").normalize().path
    systemProperty("jna.library.path", libDir)
    testLogging {
        events("passed", "failed", "skipped")
        exceptionFormat = org.gradle.api.tasks.testing.logging.TestExceptionFormat.FULL
        showStandardStreams = true
    }
}
