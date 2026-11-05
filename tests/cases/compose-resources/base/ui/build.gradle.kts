plugins {
    kotlin("multiplatform")
    id("org.jetbrains.compose")
    id("org.jetbrains.kotlin.plugin.compose")
}

kotlin {
    jvm()
    sourceSets {
        commonMain.dependencies { implementation(compose.components.resources) }
        commonTest.dependencies { implementation(kotlin("test")) }
    }
}
