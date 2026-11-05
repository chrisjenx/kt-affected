package demo

import dev.zacsweers.metro.createGraph
import kotlin.test.Test
import kotlin.test.assertEquals

class AuditTest {
    private val graph: AppGraph = createGraph()

    @Test
    fun records() {
        assertEquals("audit:x", graph.audit.record("x"))
    }
}
