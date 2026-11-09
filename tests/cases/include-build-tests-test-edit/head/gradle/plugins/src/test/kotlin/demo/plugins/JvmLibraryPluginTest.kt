package demo.plugins

import kotlin.test.Test
import kotlin.test.assertTrue

class JvmLibraryPluginTest {
    @Test
    fun appliesKotlin() {
        assertTrue(JvmLibraryPlugin() is org.gradle.api.Plugin<*>)
    }
}
