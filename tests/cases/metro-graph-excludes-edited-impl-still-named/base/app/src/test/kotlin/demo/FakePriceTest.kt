package demo

import dev.zacsweers.metro.createGraph
import kotlin.test.Test
import kotlin.test.assertEquals

class FakePriceTest {
    private val graph: FakeGraph = createGraph()

    @Test
    fun prices() {
        assertEquals(1, graph.pricing.price("abc")); H1.a()
    }
}
