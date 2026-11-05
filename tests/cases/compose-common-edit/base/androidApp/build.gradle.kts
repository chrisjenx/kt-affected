plugins {
    id("com.android.application")
    kotlin("android")
    id("org.jetbrains.kotlin.plugin.compose")
}

android { namespace = "demo.android" }

dependencies {
    implementation(project(":shared"))
    testImplementation(kotlin("test"))
}
