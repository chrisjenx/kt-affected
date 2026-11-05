plugins {
    id("com.android.application")
    kotlin("android")
}

android {
    namespace = "demo.app"
    testOptions { unitTests.isIncludeAndroidResources = true }
}

dependencies { testImplementation(kotlin("test")) }
