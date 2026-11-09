package demo.plugins

import java.io.File
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertTrue

class BuildVersionTest {
    @Test
    fun localWithoutBuildId() {
        assertEquals("local", BuildVersion.of(null))
    }

    @Test
    fun deployPassesTheBuildId() {
        val workflow = File(System.getProperty("user.dir")).resolve("../../.github/workflows/deploy.yml")
        assertTrue(workflow.readText().contains("-PbuildId="))
    }
}
