plugins {
    id("com.android.application")
    kotlin("android")
}

android { namespace = "demo.app" }

dependencies { testImplementation(kotlin("test")) }
