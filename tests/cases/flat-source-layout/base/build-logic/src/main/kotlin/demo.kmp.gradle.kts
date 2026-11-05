plugins { kotlin("multiplatform") }

kotlin {
    jvm()
    sourceSets.all {
        val dir = if (name.endsWith("Main")) "src" else "test"
        val platform = name.removeSuffix("Main").removeSuffix("Test")
        kotlin.setSrcDirs(listOf("$platform/$dir"))
    }
}
