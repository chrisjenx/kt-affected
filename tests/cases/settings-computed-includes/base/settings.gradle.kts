rootProject.name = "shop"

fun includeProjects(directory: File, path: String, maxDepth: Int = 1) {
    directory.listFiles().orEmpty().sorted().forEach { file ->
        if (file.isDirectory) {
            val newPath = "$path:${file.name}"
            if (File(file, "build.gradle.kts").exists()) {
                include(newPath)
            } else if (maxDepth > 0) {
                includeProjects(file, newPath, maxDepth - 1)
            }
        }
    }
}

includeProjects(File(rootDir, "books"), ":books")
includeProjects(File(rootDir, "orders"), ":orders")
