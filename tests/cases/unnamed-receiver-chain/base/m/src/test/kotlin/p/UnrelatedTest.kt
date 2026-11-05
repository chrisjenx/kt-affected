package p

import kotlin.test.Test
import kotlin.test.assertEquals

class UnrelatedTest {
    @Test
    fun countsLines() = assertEquals(3, Wiring.unrelated().lines())
}
