plugins { kotlin("multiplatform") }

kotlin {
    jvm()
    listOf(iosArm64(), iosSimulatorArm64()).forEach { it.binaries.framework { baseName = "Shared" } }

    sourceSets {
        commonTest.dependencies { implementation(kotlin("test")) }
    }
}
