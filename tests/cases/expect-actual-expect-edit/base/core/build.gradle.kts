plugins { kotlin("multiplatform") }

kotlin {
    jvm()
    iosSimulatorArm64()
    wasmJs { browser() }

    sourceSets {
        commonTest.dependencies { implementation(kotlin("test")) }
    }
}
