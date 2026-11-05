package demo

import kotlin.test.Test
import kotlin.test.assertEquals

class HolderTest {
    private val graph = object : AppGraph {
        override val pricing: Pricing = PricingImpl()
    }

    @Test
    fun holds() {
        assertEquals("USD", graph.pricing.currency())
    }
}
