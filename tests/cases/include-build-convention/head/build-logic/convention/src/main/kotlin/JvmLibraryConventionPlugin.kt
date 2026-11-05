import org.gradle.api.Plugin
import org.gradle.api.Project

class JvmLibraryConventionPlugin : Plugin<Project> {
    override fun apply(target: Project) {
        target.pluginManager.apply("org.jetbrains.kotlin.jvm")
        target.tasks.withType(org.gradle.api.tasks.testing.Test::class.java).configureEach { it.maxParallelForks = 2 }
    }
}
