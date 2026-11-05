package demo.app

import kotlin.test.Test
import kotlin.test.assertEquals

class PricingTest {
    @Test
    fun discounts() {
        assertEquals(10, Pricing().discount(100))
    }
}
