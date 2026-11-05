plugins {
    id("com.android.application")
    kotlin("android")
}

android {
    namespace = "demo.app"
    flavorDimensions += "store"
    productFlavors {
        create("free") {
            dimension = "store"
            isDefault = true
        }
        create("paid") { dimension = "store" }
    }
}

dependencies { testImplementation(kotlin("test")) }
