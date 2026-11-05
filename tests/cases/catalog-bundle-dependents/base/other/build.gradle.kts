plugins { kotlin("jvm") }

dependencies {
    // libs.okio.core is not used here
    implementation(libs.okio.core.fake)
    testImplementation(kotlin("test"))
}
