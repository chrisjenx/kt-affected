plugins { `kotlin-dsl` }

gradlePlugin {
    plugins.create("jvmLibrary") {
        id = "demo.jvm-library"
        implementationClass = "demo.plugins.JvmLibraryPlugin"
    }
}

tasks.register<Test>("fastTest") {
    testClassesDirs = sourceSets.test.get().output.classesDirs
    classpath = sourceSets.test.get().runtimeClasspath
    filter.excludeTestsMatching("*IT")
}
