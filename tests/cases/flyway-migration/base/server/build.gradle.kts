plugins { kotlin("jvm") }

dependencies {
    implementation("org.jetbrains.exposed:exposed-core:0.55.0")
    testImplementation("org.testcontainers:postgresql:1.20.0")
    testImplementation(kotlin("test"))
}
