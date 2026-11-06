plugins { kotlin("jvm") }

dependencies {
    testImplementation(kotlin("test"))
    testImplementation(libs.test.konsist)
}
