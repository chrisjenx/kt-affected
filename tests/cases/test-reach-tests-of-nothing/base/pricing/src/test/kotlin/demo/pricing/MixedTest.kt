package demo.pricing

import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertTrue

class MixedTest {
    private val pricing = Pricing(10)

    @Test
    fun usesPricing() {
        assertEquals(110, pricing.total(100))
    }

    @Test
    fun testsNothing() {
        assertTrue(listOf(1).isNotEmpty())
    }
}
