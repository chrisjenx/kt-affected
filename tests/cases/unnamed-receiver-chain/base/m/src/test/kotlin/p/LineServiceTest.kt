package p

import kotlin.test.Test
import kotlin.test.assertEquals

class LineServiceTest {
    @Test
    fun countsLines() = assertEquals(8, Wiring.service().lines(Tx()))
}
