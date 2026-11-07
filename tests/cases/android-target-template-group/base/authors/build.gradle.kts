plugins {
    kotlin("multiplatform")
    id("com.android.library")
}

kotlin {
    androidTarget()
    jvm()
    wasmJs { browser() }

    sourceSets {
        val sharedJvmMain by creating { dependsOn(commonMain.get()) }
        val sharedJvmTest by creating { dependsOn(commonTest.get()) }
        androidMain { dependsOn(sharedJvmMain) }
        jvmMain { dependsOn(sharedJvmMain) }
        androidUnitTest { dependsOn(sharedJvmTest) }
        jvmTest { dependsOn(sharedJvmTest) }
        commonTest.dependencies { implementation(kotlin("test")) }
    }
}

android { namespace = "demo.authors" }
