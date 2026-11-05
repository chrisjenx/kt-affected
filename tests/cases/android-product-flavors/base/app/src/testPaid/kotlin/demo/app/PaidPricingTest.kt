package demo.app

import kotlin.test.Test
import kotlin.test.assertEquals

class PaidPricingTest {
    @Test
    fun discounts() {
        assertEquals(1, Pricing().discount(10))
    }
}
