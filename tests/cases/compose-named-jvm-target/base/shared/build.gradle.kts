plugins {
    kotlin("multiplatform")
    id("org.jetbrains.compose")
    id("org.jetbrains.kotlin.plugin.compose")
}

kotlin {
    jvm("desktop")
    wasmJs { browser() }

    sourceSets {
        commonTest.dependencies { implementation(kotlin("test")) }
    }
}
