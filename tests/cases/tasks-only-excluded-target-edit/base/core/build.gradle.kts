plugins {
    kotlin("multiplatform")
}

kotlin {
    jvm()
    iosSimulatorArm64()
    js { browser() }

    sourceSets {
        commonTest.dependencies {
            implementation(kotlin("test"))
        }
    }
}
