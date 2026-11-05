plugins { `kotlin-dsl` }

gradlePlugin {
    plugins {
        register("jvmLibrary") {
            id = "demo.jvm.library"
            implementationClass = "JvmLibraryConventionPlugin"
        }
    }
}
