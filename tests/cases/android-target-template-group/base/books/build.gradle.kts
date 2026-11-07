plugins {
    kotlin("multiplatform")
    id("com.android.library")
}

kotlin {
    applyDefaultHierarchyTemplate {
        group("nonWeb") {
            withAndroidTarget()
            withJvm()
        }
    }
    androidTarget()
    jvm()
    wasmJs { browser() }

    sourceSets {
        commonTest.dependencies { implementation(kotlin("test")) }
    }
}

android { namespace = "demo.books" }
