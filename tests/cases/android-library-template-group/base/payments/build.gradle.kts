import com.android.build.api.withAndroid

plugins {
    kotlin("multiplatform")
    id("com.android.kotlin.multiplatform.library")
}

kotlin {
    applyDefaultHierarchyTemplate {
        group("nonWeb") {
            withAndroid()
            withJvm()
        }
    }
    androidLibrary {
        namespace = "demo.payments"
        withHostTest {}
    }
    jvm()
    wasmJs { browser() }

    sourceSets {
        val nonWebMain = getByName("nonWebMain") { dependsOn(commonMain.get()) }
        val nonWebTest = getByName("nonWebTest") { dependsOn(commonTest.get()) }
        androidMain { dependsOn(nonWebMain) }
        commonTest.dependencies { implementation(kotlin("test")) }
    }
}
