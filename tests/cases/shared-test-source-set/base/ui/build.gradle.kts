plugins { kotlin("multiplatform") }

kotlin {
    jvm()
    wasmJs { browser() }
    iosSimulatorArm64()

    sourceSets {
        val mockedUiTest by creating { dependsOn(commonTest.get()) }
        jvmTest { dependsOn(mockedUiTest) }
        wasmJsTest { dependsOn(mockedUiTest) }
        commonTest.dependencies { implementation(kotlin("test")) }
    }
}
