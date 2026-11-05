package demo

import kotlin.test.Test
import kotlin.test.assertEquals

class ScreenTest {
    @Test
    fun renders() {
        assertEquals("<h1>Hi</h1>", Screen("Hi").render())
    }
}
