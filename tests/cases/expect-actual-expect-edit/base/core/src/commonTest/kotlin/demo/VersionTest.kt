package demo

import kotlin.test.Test
import kotlin.test.assertEquals

class VersionTest {
    @Test
    fun version() {
        assertEquals(3, version())
    }
}
