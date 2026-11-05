plugins { kotlin("jvm") }

kotlin { jvmToolchain(17) }

tasks.withType<Test>().configureEach {
    jvmArgs("-XX:TieredStopAtLevel=1")
}
