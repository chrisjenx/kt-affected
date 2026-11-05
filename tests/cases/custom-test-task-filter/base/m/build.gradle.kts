plugins { kotlin("jvm") }

val buckets = listOf("demo.logic.*")

tasks.register<Test>("captureScreens") {
    testClassesDirs = sourceSets.test.get().output.classesDirs
    classpath = sourceSets.test.get().runtimeClasspath
    filter.includeTestsMatching("demo.screens.*")
    maxParallelForks = 1
}

tasks.register<Test>("logicOnly") {
    testClassesDirs = sourceSets.test.get().output.classesDirs
    classpath = sourceSets.test.get().runtimeClasspath
    filter {
        includeTestsMatching("demo.*")
        excludeTestsMatching("demo.screens.*")
    }
}

tasks.register<Test>("bucketed") {
    testClassesDirs = sourceSets.test.get().output.classesDirs
    classpath = sourceSets.test.get().runtimeClasspath
    buckets.forEach { filter.includeTestsMatching(it) }
}
