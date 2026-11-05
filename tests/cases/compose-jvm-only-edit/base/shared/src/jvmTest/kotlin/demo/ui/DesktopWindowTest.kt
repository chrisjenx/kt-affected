package demo.ui

import kotlin.test.Test
import kotlin.test.assertEquals

class DesktopWindowTest {
    @Test
    fun title() {
        assertEquals("Demo on desktop", windowTitle())
    }
}
