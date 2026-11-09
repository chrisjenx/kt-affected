package demo.plugins

import kotlin.test.Test
import kotlin.test.assertTrue

class JvmLibraryPluginTest {
    @Test
    fun applies() {
        assertTrue(JvmLibraryPlugin() is org.gradle.api.Plugin<*>)
    }
}
