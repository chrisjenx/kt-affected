package demo.pricing

import kotlin.test.Test
import kotlin.test.assertEquals

class PriceTest {
    @Test
    fun formatsDollars() = assertEquals("$1", Money(100).format())
}
