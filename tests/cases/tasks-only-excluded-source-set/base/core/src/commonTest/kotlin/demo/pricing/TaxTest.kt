package demo.pricing

import kotlin.test.Test
import kotlin.test.assertEquals

class TaxTest {
    @Test
    fun tenPercent() = assertEquals(10, tax(100))
}
