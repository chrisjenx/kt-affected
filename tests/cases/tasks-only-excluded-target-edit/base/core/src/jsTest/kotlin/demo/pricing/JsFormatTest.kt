package demo.pricing

import kotlin.test.Test
import kotlin.test.assertEquals

class JsFormatTest {
    @Test
    fun formats() = assertEquals("$1", JsFormat().format(100))
}
