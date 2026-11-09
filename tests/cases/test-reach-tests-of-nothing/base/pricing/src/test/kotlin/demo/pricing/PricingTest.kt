package demo.pricing

import kotlin.test.Test
import kotlin.test.assertEquals

class PricingTest {
    @Test
    fun addsTax() {
        assertEquals(120, Pricing(20).total(100))
    }
}
