package demo.plugins

import kotlin.test.Test
import kotlin.test.assertEquals

class BuildVersionIT {
    @Test
    fun localBuildsAreDev() {
        assertEquals("dev", BuildVersion.of(null))
    }
}
