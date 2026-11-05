plugins { id("demo.kmp") }

kotlin {
    sourceSets {
        commonTest.dependencies { implementation(kotlin("test")) }
    }
}
