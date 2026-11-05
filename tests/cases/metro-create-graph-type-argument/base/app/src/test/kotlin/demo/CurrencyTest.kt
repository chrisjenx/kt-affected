package demo

import dev.zacsweers.metro.createGraph
import kotlin.test.Test
import kotlin.test.assertEquals

class CurrencyTest {
    private val graph = createGraph<AppGraph>()

    @Test
    fun currency() {
        assertEquals("USD", graph.pricing.currency())
    }
}
