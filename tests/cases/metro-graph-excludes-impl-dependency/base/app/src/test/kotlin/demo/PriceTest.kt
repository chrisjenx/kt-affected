package demo

import dev.zacsweers.metro.createGraph
import kotlin.test.Test
import kotlin.test.assertEquals

class PriceTest {
    private val graph: AppGraph = createGraph()

    @Test
    fun prices() {
        assertEquals(300, graph.pricing.price("abc"))
    }
}
