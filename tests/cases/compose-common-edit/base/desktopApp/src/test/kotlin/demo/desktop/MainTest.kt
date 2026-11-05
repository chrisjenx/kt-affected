package demo.desktop

import kotlin.test.Test
import kotlin.test.assertEquals

class TitleTest {
    @Test
    fun hasTheWindowTitle() {
        assertEquals("Demo on desktop", title())
    }
}

class ContentTest {
    @Test
    fun renders() {
        content()
    }
}
