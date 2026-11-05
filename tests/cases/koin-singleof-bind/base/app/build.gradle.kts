plugins { kotlin("jvm") }

dependencies {
    implementation("io.insert-koin:koin-core:4.0.0")
    testImplementation("io.insert-koin:koin-test:4.0.0")
    testImplementation(kotlin("test"))
}
