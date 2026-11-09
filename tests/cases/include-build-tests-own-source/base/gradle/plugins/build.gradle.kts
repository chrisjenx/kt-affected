plugins { `kotlin-dsl` }

gradlePlugin {
    plugins.create("jvmLibrary") {
        id = "demo.jvm-library"
        implementationClass = "demo.plugins.JvmLibraryPlugin"
    }
}
