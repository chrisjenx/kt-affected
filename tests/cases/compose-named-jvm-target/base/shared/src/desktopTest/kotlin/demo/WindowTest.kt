package demo

import kotlin.test.Test
import kotlin.test.assertEquals

class WindowTest {
    @Test
    fun title() {
        assertEquals("Window #1", windowTitle())
    }
}
