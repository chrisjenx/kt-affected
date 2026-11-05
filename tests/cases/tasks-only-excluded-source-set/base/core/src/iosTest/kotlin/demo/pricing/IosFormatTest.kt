package demo.pricing

import kotlin.test.Test
import kotlin.test.assertEquals

class IosFormatTest {
    @Test
    fun formats() = assertEquals("$1", IosFormat().format(100))
}
