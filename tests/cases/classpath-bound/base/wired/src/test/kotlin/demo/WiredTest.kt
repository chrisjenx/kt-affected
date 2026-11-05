package demo

import kotlin.test.Test

class WiredTest {
    private val graph: WiredGraph = makeWiredGraph()

    @Test
    fun runs() {
        graph.schedule.slots(1)
    }
}
