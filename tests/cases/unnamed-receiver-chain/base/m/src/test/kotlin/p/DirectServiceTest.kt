package p

import kotlin.test.Test
import kotlin.test.assertEquals

class DirectServiceTest {
    private val service: LineService = Wiring.service()

    @Test
    fun countsLines() = assertEquals(8, service.lines(Tx()))
}
