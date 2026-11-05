package demo

import kotlin.test.Test

class ApiOnlyTest {
    private val graph: ApiOnlyGraph = makeApiOnlyGraph()

    @Test
    fun runs() {
        graph.schedule.slots(1)
    }
}
